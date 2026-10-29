#!/usr/bin/env bash
# Downloads and prepares the search index datasets into a directory and
# writes its manifest.tsv, for:
#
#   benches/index_datasets.sh ~/index-datasets
#   cargo bench --features encodings --bench compression_ratio -- index_sizes ~/index-datasets/manifest.tsv
#
# About 15 GB of downloads and 40 GB on disk. Every step skips its output if it
# already exists, so rerunning only fetches what's missing. Needs curl, git, jq,
# unzip and a built compression_ratio bench (for Parquet columns).
set -euo pipefail
dir="${1:?usage: index_datasets.sh <dir>}"
mkdir -p "${dir}"/{raw,prose,code,logs,columns}
dir="$(cd "${dir}" && pwd)"
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --release --features encodings --bench compression_ratio --manifest-path "${repo}/Cargo.toml" -q
bench="$(ls -t "${repo}"/target/release/deps/compression_ratio-* | grep -E '/compression_ratio-[0-9a-f]+$' | head -1)"
manifest=()

# fetch <url> <file>: download into raw/ unless it's there.
fetch() {
  [[ -s "${dir}/raw/$2" ]] && return
  curl -sSfL --retry 3 -o "${dir}/raw/$2.part" "$1"
  mv "${dir}/raw/$2.part" "${dir}/raw/$2"
}
# add <kind> <name> <lines|tsv|paragraphs|files> <path relative to dir>
add() { manifest+=("$1	$2	$3	$4"); }
# made <file>: whether a prepared file already exists.
made() { [[ -s "${dir}/$1" ]]; }

# Prose: one document per line, or per paragraph for books.
fetch https://msmarco.z22.web.core.windows.net/msmarcoranking/collection.tar.gz msmarco.tar.gz
made prose/msmarco.tsv || tar -xzOf "${dir}/raw/msmarco.tar.gz" collection.tsv > "${dir}/prose/msmarco.tsv"
add Prose "MS MARCO passages" tsv prose/msmarco.tsv
fetch http://mattmahoney.net/dc/enwik8.zip enwik8.zip
made prose/enwik8 || unzip -qo "${dir}/raw/enwik8.zip" -d "${dir}/prose"
add Prose "enwik8 (Wikipedia XML)" lines prose/enwik8
fetch https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip silesia.zip
for f in dickens webster; do
  made "prose/${f}" || unzip -qo "${dir}/raw/silesia.zip" "${f}" -d "${dir}/prose"
  add Prose "Silesia \`${f}\`" paragraphs "prose/${f}"
done
fetch https://huggingface.co/datasets/allenai/c4/resolve/main/en/c4-train.00000-of-01024.json.gz c4.json.gz
made prose/c4.txt || gzip -dc "${dir}/raw/c4.json.gz" | jq -r '.text | gsub("[\r\n]+"; " ")' > "${dir}/prose/c4.txt"
add Prose "C4 (web pages, one shard)" lines prose/c4.txt
fetch https://mcauleylab.ucsd.edu/public_datasets/data/amazon_2023/raw/review_categories/Video_Games.jsonl.gz amazon-reviews.jsonl.gz
made prose/amazon-reviews.txt || gzip -dc "${dir}/raw/amazon-reviews.jsonl.gz" | jq -r 'select(.text != "") | .text | gsub("[\r\n]+|<br */?>"; " ")' > "${dir}/prose/amazon-reviews.txt"
add Prose "Amazon reviews (Video Games)" lines prose/amazon-reviews.txt
fetch https://huggingface.co/datasets/emozilla/pg19/resolve/main/data/test-00000-of-00001-29a571947c0b5ccc.parquet pg19-test.parquet
made prose/text.txt || "${bench}" columns "${dir}/raw/pg19-test.parquet" "${dir}/prose" text --books
made prose/pg19.txt || mv "${dir}/prose/text.txt" "${dir}/prose/pg19.txt"
add Prose "PG-19 books (test split)" paragraphs prose/pg19.txt

# Code: shallow clones at a tag, one document per source file.
repos=(
  "torvalds/linux v6.12 linux|Linux v6.12 (C)"
  "llvm/llvm-project llvmorg-19.1.0 llvm-project|LLVM 19.1.0 (C++)"
  "dotnet/runtime v9.0.0 dotnet-runtime|.NET runtime 9.0.0 (C#)"
  "nodejs/node v23.0.0 node|Node v23.0.0 (C++, JS)"
  "tensorflow/tensorflow v2.18.0 tensorflow|TensorFlow 2.18.0 (C++, Python)"
  "elastic/elasticsearch v8.16.0 elasticsearch|Elasticsearch 8.16.0 (Java)"
  "kubernetes/kubernetes v1.32.0 kubernetes|Kubernetes v1.32.0 (Go)"
  "swiftlang/swift swift-6.0-RELEASE swift|Swift 6.0 (C++, Swift)"
  "rust-lang/rust 1.84.0 rust|Rust 1.84.0 (Rust)"
  "postgres/postgres REL_17_0 postgres|Postgres 17.0 (C)"
  "symfony/symfony v7.2.0 symfony|Symfony v7.2.0 (PHP)"
  "python/cpython v3.13.0 cpython|CPython v3.13.0 (C, Python)"
  "golang/go go1.23.0 go|Go 1.23.0 (Go)"
  "microsoft/vscode 1.96.0 vscode|VS Code 1.96.0 (TypeScript)"
  "spring-projects/spring-framework v6.2.0 spring-framework|Spring Framework v6.2.0 (Java)"
  "django/django 5.1 django|Django 5.1 (Python)"
  "facebook/react v19.0.0 react|React 19.0.0 (JS)"
  "rails/rails v8.0.0 rails|Rails 8.0.0 (Ruby)"
)
pids=()
for r in "${repos[@]}"; do
  read -r slug tag name <<<"${r%%|*}"
  [[ -d "${dir}/code/${name}" ]] || git -c advice.detachedHead=false clone -q --depth 1 --branch "${tag}" \
    "https://github.com/${slug}.git" "${dir}/code/${name}" &
  pids+=($!)
  add Code "${r#*|}" files "code/${name}"
done
wait "${pids[@]}"

# Logs: loghub systems, one file per system, one document per line.
loghub=(Android_v1.zip Apache.tar.gz BGL.zip HDFS_v1.zip HPC.zip Hadoop.zip HealthApp.tar.gz Linux.tar.gz
  Mac.tar.gz OpenStack.tar.gz Proxifier.tar.gz SSH.tar.gz Spark.tar.gz Zookeeper.tar.gz)
for f in "${loghub[@]}"; do
  system="${f%%.*}"
  if ! made "logs/${system}.log"; then
    fetch "https://zenodo.org/records/8196385/files/${f}?download=1" "${f}"
    rm -rf "${dir}/raw/${system}" && mkdir -p "${dir}/raw/${system}"
    case "${f}" in
      *.zip) unzip -qo "${dir}/raw/${f}" -d "${dir}/raw/${system}" ;;
      *) tar -xzf "${dir}/raw/${f}" -C "${dir}/raw/${system}" ;;
    esac
    find "${dir}/raw/${system}" -type f \( -name '*.log' -o -name '*.txt' -o -name '*.out' \) \
      ! -name '*_2k*' ! -iname '*readme*' ! -name '*label*' -print0 | sort -z | xargs -0 cat > "${dir}/logs/${system}.log"
  fi
  add Logs "loghub \`${system%_v1}\`" lines "logs/${system}.log"
done
# Thunderbird is 30 GB uncompressed: keep its first GiB.
made logs/Thunderbird.log || { curl -sSfL "https://zenodo.org/records/8196385/files/Thunderbird.tar.gz?download=1" \
  | tar -xzO 2>/dev/null | head -c 1073741824 > "${dir}/logs/Thunderbird.log" || true; }
add Logs "loghub \`Thunderbird\` (first GiB)" lines logs/Thunderbird.log
fetch http://ita.ee.lbl.gov/traces/NASA_access_log_Jul95.gz nasa-jul95.gz
fetch http://ita.ee.lbl.gov/traces/NASA_access_log_Aug95.gz nasa-aug95.gz
made logs/nasa.log || gzip -dc "${dir}/raw/nasa-jul95.gz" "${dir}/raw/nasa-aug95.gz" > "${dir}/logs/nasa.log"
add Logs "NASA HTTP access, Jul-Aug 1995" lines logs/nasa.log

# Relational columns: one document per non-empty value.
fetch https://datasets.clickhouse.com/hits_compatible/athena_partitioned/hits_0.parquet hits_0.parquet
made columns/Title.txt || "${bench}" columns "${dir}/raw/hits_0.parquet" "${dir}/columns" Title URL Referer SearchPhrase
for c in Title URL Referer SearchPhrase; do add "Relational Columns" "ClickBench \`${c}\`" lines "columns/${c}.txt"; done
fetch https://datasets.imdbws.com/title.basics.tsv.gz imdb-title.basics.tsv.gz
fetch https://datasets.imdbws.com/name.basics.tsv.gz imdb-name.basics.tsv.gz
fetch https://datasets.imdbws.com/title.akas.tsv.gz imdb-title.akas.tsv.gz
made columns/imdb_primaryTitle.txt || gzip -dc "${dir}/raw/imdb-title.basics.tsv.gz" | tail -n +2 | cut -f3 > "${dir}/columns/imdb_primaryTitle.txt"
made columns/imdb_primaryName.txt || gzip -dc "${dir}/raw/imdb-name.basics.tsv.gz" | tail -n +2 | cut -f2 > "${dir}/columns/imdb_primaryName.txt"
made columns/imdb_akas_title.txt || gzip -dc "${dir}/raw/imdb-title.akas.tsv.gz" | tail -n +2 | cut -f3 > "${dir}/columns/imdb_akas_title.txt"
add "Relational Columns" "IMDB \`primaryTitle\`" lines columns/imdb_primaryTitle.txt
add "Relational Columns" "IMDB \`primaryName\`" lines columns/imdb_primaryName.txt
add "Relational Columns" "IMDB \`title.akas\` titles" lines columns/imdb_akas_title.txt
fetch https://dblp.org/xml/dblp.xml.gz dblp.xml.gz
if ! made columns/dblp_title.txt; then
  gzip -dc "${dir}/raw/dblp.xml.gz" | grep -o '<title>.*</title>' | sed -e 's/<[^>]*>//g' > "${dir}/columns/dblp_title.txt"
  gzip -dc "${dir}/raw/dblp.xml.gz" | grep -o '<author[^>]*>[^<]*</author>' | sed -e 's/<[^>]*>//g' > "${dir}/columns/dblp_author.txt"
fi
add "Relational Columns" "DBLP paper titles" lines columns/dblp_title.txt
add "Relational Columns" "DBLP authors" lines columns/dblp_author.txt
fetch https://static.openfoodfacts.org/data/en.openfoodfacts.org.products.csv.gz openfoodfacts.tsv.gz
if ! made columns/openfoodfacts_product_name.txt; then
  gzip -dc "${dir}/raw/openfoodfacts.tsv.gz" | awk -F'\t' 'NR == 1 { for (i = 1; i <= NF; i++) if ($i == "product_name") c = i; next }
    $c != "" { print $c }' > "${dir}/columns/openfoodfacts_product_name.txt"
fi
add "Relational Columns" "Open Food Facts \`product_name\`" lines columns/openfoodfacts_product_name.txt
fetch https://mcauleylab.ucsd.edu/public_datasets/data/amazon_2023/raw/meta_categories/meta_Video_Games.jsonl.gz amazon-meta.jsonl.gz
made columns/amazon_product_title.txt || gzip -dc "${dir}/raw/amazon-meta.jsonl.gz" | jq -r 'select(.title != "") | .title | gsub("[\r\n]+"; " ")' > "${dir}/columns/amazon_product_title.txt"
made columns/amazon_review_title.txt || gzip -dc "${dir}/raw/amazon-reviews.jsonl.gz" | jq -r 'select(.title != "") | .title | gsub("[\r\n]+"; " ")' > "${dir}/columns/amazon_review_title.txt"
add "Relational Columns" "Amazon product titles (Video Games)" lines columns/amazon_product_title.txt
add "Relational Columns" "Amazon review titles (Video Games)" lines columns/amazon_review_title.txt

printf '%s\n' "# kind	name	split	path" "${manifest[@]}" > "${dir}/manifest.tsv"
echo "wrote ${dir}/manifest.tsv (${#manifest[@]} datasets)"

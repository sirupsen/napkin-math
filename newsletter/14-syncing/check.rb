require 'mysql2'
require 'time'

client_a = Mysql2::Client.new(host: "localhost", username: "root", database: 'napkin')
# client_b = Mysql2::Client.new(host: "localhost", username: "root", database: 'napkin')

count, batch_size, start = 100_000_000, 10_000, Time.now
# iterations = count / batch_size
iterations = 100
max_id_from_last_batch = 0

# TODO: array of sql queries, write 100 batches into csvs

queries = []

# queries << ["iteration_1", <<~SQL
#   SELECT *
#   FROM `table`
#   ORDER BY id ASC
#   LIMIT %{limit}
#   OFFSET %{offset}
# SQL
# ]

# queries << ["iteration_2", <<~SQL
#   SELECT * FROM `table`
#   WHERE id <= (SELECT id FROM `table` LIMIT 1 OFFSET %{offset})
#   ORDER BY id ASC
#   LIMIT %{limit}
# SQL
# ]

# queries << ["iteration_4", <<~SQL
#   SELECT * FROM `table`
#   WHERE id > %{max_id_from_last_batch}
#   ORDER BY id ASC
#   LIMIT %{limit}
# SQL
# ]

# queries << ["iteration_5", <<~SQL
# SELECT max(id) as id, MD5(CONCAT(
#   MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(t.id))))),
#   MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(t.updated_at)))))
# )) as checksum
# FROM (
#    SELECT id, updated_at FROM `table`
#    WHERE id > %{max_id_from_last_batch}
#    LIMIT %{limit}
# ) t
# SQL
# ]

queries << ["iteration_6", <<~SQL
SELECT max(id) as id, SUM(UNIX_TIMESTAMP(updated_at)) as checksum
FROM `table`
WHERE id < (SELECT id FROM `table` WHERE id > %{max_id_from_last_batch} LIMIT 1 OFFSET 10000)
  AND id > %{max_id_from_last_batch}
SQL
]

def render_plot(title:, xlabel: "Batch", ylabel: "Time in ms", output:, input:)
  plot_script = <<~GNUPLOT
    set title '%{title}'
    set ylabel '%{ylabel}'
    set xlabel '%{xlabel}'
    set grid # Show the grid
    set term png
    set output '%{output}'
    plot '%{input}' title ''
  GNUPLOT

  plot_script = plot_script % { input: input, output: output, title: title, xlabel: xlabel, ylabel: ylabel }
  plot_script_name = "#{title}.gnuplot"
  File.open(plot_script_name, "w+") { |f| f.write(plot_script) }
  system("gnuplot #{plot_script_name}")
end

queries.each do |(name, sql)|
  csv_file = "#{name}.csv"
  file = File.open(csv_file, "w+")

  (1..iterations).each do |i|
    before_batch = Time.now

    limit = 10_000
    offset = (i - 1) * batch_size

    final_sql = sql % { limit: limit, offset: offset, max_id_from_last_batch: max_id_from_last_batch }
    res_a = client_a.query(final_sql).to_a

    batch_time_ms = ((Time.now - before_batch) * 1000).round(1)

    # TODO: csv
    $stdout.puts "name=#{name} batch=#{i}/#{iterations} batch_time=#{batch_time_ms}ms limit=#{limit} offset=#{offset} max_id=#{max_id_from_last_batch}"

    file.write("#{i}, #{batch_time_ms}\n")
    max_id_from_last_batch = res_a.last["id"]
  end
  file.close
  render_plot(input: csv_file, output: "#{name}.png", title: name)
end

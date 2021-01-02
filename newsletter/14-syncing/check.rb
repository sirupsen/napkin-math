require 'mysql2'
require 'time'
require 'socket'

client_a = Mysql2::Client.new(host: "localhost", username: "root", database: 'napkin')
# client_b = Mysql2::Client.new(host: "localhost", username: "root", database: 'napkin')

count, batch_size, start = 100_000_000, 10_000, Time.now
# iterations = count / batch_size
iterations = 100
max_id_from_last_batch = 0

# TODO: array of sql queries, write 100 batches into csvs

queries = []

queries << ["iteration_1", <<~SQL
  SELECT *
  FROM `table`
  ORDER BY id ASC
  LIMIT %{limit}
  OFFSET %{offset}
SQL
]

queries << ["iteration_2", <<~SQL
  SELECT * FROM `table`
  WHERE id <= (SELECT id FROM `table` LIMIT 1 OFFSET %{offset})
  ORDER BY id ASC
  LIMIT %{limit}
SQL
]

queries << ["iteration_4", <<~SQL
  SELECT * FROM `table`
  WHERE id > %{max_id_from_last_batch}
  ORDER BY id ASC
  LIMIT %{limit}
SQL
]

queries << ["iteration_5", <<~SQL
SELECT max(id) as id, MD5(CONCAT(
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(id))))),
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(data1))))),
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(data2))))),
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(data3))))),
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(data4))))),
  MD5(GROUP_CONCAT(UNHEX(MD5(COALESCE(updated_at)))))
)) as checksum
FROM `table`
WHERE id < (SELECT id FROM `table` WHERE id > %{max_id_from_last_batch} LIMIT 1 OFFSET 10000)
  AND id > %{max_id_from_last_batch}
SQL
]

queries << ["iteration_6", <<~SQL
SELECT max(id) as id, SUM(UNIX_TIMESTAMP(updated_at)) as checksum
FROM `table`
WHERE id < (SELECT id FROM `table` WHERE id > %{max_id_from_last_batch} LIMIT 1 OFFSET 10000)
  AND id > %{max_id_from_last_batch}
SQL
]

queries << ["iteration_7", <<~SQL
SELECT max(id) as id, SUM(UNIX_TIMESTAMP(updated_at)) as checksum
FROM `table`
FORCE INDEX (index_table_id_updated_at)
WHERE id < (SELECT id FROM `table` FORCE INDEX (index_table_id) WHERE id > %{max_id_from_last_batch} LIMIT 1 OFFSET 10000)
  AND id > %{max_id_from_last_batch}
SQL
]


queries.each do |(name, sql)|
  10.times { GC.start }
  sleep(0.1)
  system("sudo purge") # clear disk cache
  system("mysql.server restart") # restart mysql to flush buffer pools
  csv_file = "#{name}.csv"

  loop do
    begin
      TCPSocket.open("localhost", 3306)
      break
    rescue Errno::ECONNREFUSED
      retry
    end
  end

  client_a = Mysql2::Client.new(host: "localhost", username: "root", database: 'napkin')

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
end

def render_plot(title: "Checksum performance (lower is better)", xlabel: "Batch", ylabel: "Time in ms", output:, input: [], plot: [], expectation:)
  plot_script = <<~GNUPLOT
    set title '%{title}'
    set ylabel '%{ylabel}'
    set xlabel '%{xlabel}'
    set grid # Show the grid
    set term png
    set output '%{output}'
    plot \\
  GNUPLOT

  plot_script = plot_script % { output: output, title: title, xlabel: xlabel, ylabel: ylabel }

  plot_script += input.map { |input| "'#{input}' title \"#{input.chomp(".csv")}\"" }.join(", ")

  plot_script += ", #{expectation} title 'Napkin math lower bound #{input.last.chomp("csv")}' lw 3 lc 'red'"

  puts plot_script

  plot_script_name = "#{input.last.chomp(".csv")}.gnuplot"

  File.open(plot_script_name, "w+") { |f| f.write(plot_script) }
  system("gnuplot #{plot_script_name}")
end

render_plot(output: "iteration_1.png", input: ["iteration_1.csv"], expectation: 100)
render_plot(output: "iteration_2.png", input: ["iteration_1.csv", "iteration_2.csv"], expectation: 100)
render_plot(output: "iteration_4.png", input: ["iteration_1.csv", "iteration_2.csv", "iteration_4.csv"], expectation: 100)
render_plot(output: "iteration_5.png", input: ["iteration_1.csv", "iteration_2.csv", "iteration_4.csv", "iteration_5.csv"], expectation: 50)

render_plot(output: "iteration_6.png", input: ["iteration_1.csv", "iteration_2.csv", "iteration_4.csv", "iteration_5.csv", "iteration_6.csv"], expectation: 3)
render_plot(output: "iteration_6_no_1.png", input: ["iteration_2.csv", "iteration_4.csv", "iteration_5.csv", "iteration_6.csv"], plot: ["0x + 5 title 'Napkin Math'"], expectation: 3)

render_plot(output: "iteration_7.png", input: ["iteration_2.csv", "iteration_4.csv", "iteration_5.csv", "iteration_6.csv", "iteration_7.csv"], expectation: 3)
render_plot(output: "iteration_7-1.png", input: ["iteration_6.csv", "iteration_7.csv"], expectation: 3)

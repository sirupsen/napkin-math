# frozen_string_literal: true

require 'pg'
require 'mysql2'
require 'fiber_scheduler'

# conn = PG.connect(dbname: 'postgres', host: 'localhost', user: 'postgres')
conn = Mysql2::Client.new(host: '0.0.0.0', username: 'root', database: 'mysql')
p conn

conn.query('drop table if exists test_table') if ENV['TRUNCATE']
conn.query <<~QUERY
  create table if not exists test_table
  (
      id    bigint    primary key,
      text1 text      NOT NULL,
      text2 text      NOT NULL,
      int1000  bigint    not null,
      int100 bigint    not null,

      int10  bigint    not null,
      int10_2  bigint    not null
  );
QUERY

TARGET_ROWS = 10_000_000
BATCH_SIZE = 10_000

size_of_table = conn.query('select count(*) from test_table').to_a[0].values[0].to_i
rows_to_create = (size_of_table + 1..TARGET_ROWS)

one_kib = 'b' * 1024
bytes_255 = 'b' * 255

rows_to_create.each_slice(BATCH_SIZE).each_with_index do |batch, i|
  sql = +'INSERT INTO test_table (id, text1, text2, int1000, int100, int10, int10_2) VALUES '
  batch.each do |id|
    sql << "(#{id}, '#{one_kib}', '#{bytes_255}', #{rand(1000)}, #{rand(100)},"
    sql << "#{rand(10)}, #{rand(10)}),"
  end
  conn.query(sql[0..-2])

  puts "#{i + 1}/#{rows_to_create.size / BATCH_SIZE}"
end

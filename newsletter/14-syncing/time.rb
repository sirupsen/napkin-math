sum = 0
(1..100).each do |i|
  res = 100 + i * 5
  # puts "#{i}: #{res}"
  sum += res
end

puts "100 batches: #{sum}ms"

(1..10_000).each do |i|
  res = 100 + i * 5
  sum += res
end

puts "10,000 batches: #{sum}ms"

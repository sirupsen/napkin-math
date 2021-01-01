sum = 0
(1..10000).each do |i|
  res = 400 + i * 0.5
  puts res
  sum += res
end

puts sum

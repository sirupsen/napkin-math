set title 'Checksum performance'
set ylabel 'Time in ms'
set xlabel 'Batch'
set grid # Show the grid
set term png
set output 'iteration_4.png'
plot \
'iteration_1.csv' title "iteration_1", 'iteration_2.csv' title "iteration_2", 'iteration_4.csv' title "iteration_4"
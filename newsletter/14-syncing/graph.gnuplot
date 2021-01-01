set title 'Iteration 1'
set ylabel 'Time in ms'
set xlabel 'Batch number'
set grid # Show the grid
set term png
set output 'iteration_1.png'
plot 'iteration_1.csv' title ''

set title 'Checksum performance (lower is better)'
set ylabel 'Time in ms'
set xlabel 'Batch'
set grid # Show the grid
set term png
set output 'iteration_1.png'
plot \
'iteration_1.csv' title "iteration_1", 100 title 'Napkin math lower bound iteration_1.' lw 3 lc 'red'
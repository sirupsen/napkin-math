set title 'Checksum performance (lower is better)'
set ylabel 'Time in ms'
set xlabel 'Batch'
set grid # Show the grid
set term png
set output 'iteration_7-1.png'
plot \
'iteration_6.csv' title "iteration_6", 'iteration_7.csv' title "iteration_7", 3 title 'Napkin math lower bound iteration_7.' lw 3 lc 'red'
package main

import (
	"fmt"
	"math/rand"
	"testing"
)

type Cell88 struct {
	padding [80]byte
	value   float64
}

func BenchmarkMapAccess(b *testing.B) {
	var n_cells uint32 = 10_000_000

	units := make(map[uint32]*Cell88, n_cells)
	price_per_unit := make(map[uint32]*Cell88, n_cells)
	revenue := make(map[uint32]*Cell88, n_cells)

	var i uint32
	for i = 0; i < n_cells; i++ {
		units[i] = &Cell88 { value: rand.Float64() }
		price_per_unit[i] = &Cell88 { value: rand.Float64() }
		revenue[i] = &Cell88 { value: 0. }
	}

	b.ResetTimer()

	for n := 0; n < b.N; n++ {
		fmt.Println(n);
		for i = 0; i < n_cells; i++ {
			revenue[i].value = units[i].value + price_per_unit[i].value
		}
	}
}

package main

import (
	"fmt"
	"math"
	"math/rand"
	"runtime/debug"
	"time"

	"golang.org/x/text/language"
	"golang.org/x/text/message"
)

type Rank struct {
	a uint8
	b int64
}

type SmallestCell struct {
	padding [8]byte
	value   float64
}

type SmallCell struct {
	padding [24]byte
	value   float64
}

type SmallerCell struct {
	padding [56]byte
	value   float64
}

type LargeCell struct {
	padding [80]byte
	value   float64
}

type Cell float64

func main() {
	p := message.NewPrinter(language.English)
	experiments := []int{50_000_000} //, 10_000_000}
	nTests := 10                     // for gc pressure etc

	gcStats := debug.GCStats{}

	for i := 1; i <= nTests; i++ {
		for _, nCells := range experiments {
			debug.ReadGCStats(&gcStats)
      fmt.Printf("Total GC Time: %s\n", gcStats.PauseTotal.String());
      fmt.Printf("Recent GC Times: %+v\n", gcStats.Pause[0:7]);
      fmt.Printf("Total GC Runs: %d\n", gcStats.NumGC);

			p.Printf("%d cells, run %d/%d\n", nCells, i, nTests)
			// multiDimArray(nCells)
			// arrayCellValuesSmallest(nCells)
			// arrayCellValuesSmall(nCells)
			// arrayCellValuesSmaller(nCells)
			// arrayCellValues(nCells)
			arrayCellPointers(nCells)
			// arrayCellPointersPreAllocated(nCells)
			// pointerMapIntegerIndexSmallCellsValue(nCells)
			// pointerMapIntegerIndexSmallCells(nCells)
			// pointerMapIntegerIndexSmallerCells(nCells)
			// pointerMapIntegerIndex(nCells)
			// pointerMapSmallCells(nCells)
			// pointerMapSmallerCells(nCells)
			// pointerMapPreAllocate(nCells)
			pointerMap(nCells)
			// valueMap(nCells)
		}
	}
}

func multiDimArray(nCells int) {
	start := time.Now()
	fmt.Println("SUP")
	variableSize := ((8) * float64(nCells)) / math.Pow(2, 20)
	fmt.Printf("\t[]float64, %.1f MiB\n", variableSize)

	fmt.Printf("\t\tNapkin Math node generation: %.f ms\n", (variableSize*50*2)/1000)
	fmt.Printf("\t\tNapkin Math multiplication:  %.f ms\n\n", (variableSize*50*3)/1000)

	one := make([]Cell, nCells)
	two := make([]Cell, nCells)
	res := make([]Cell, nCells)

	// rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = Cell(rand.Float64())
		two[i] = Cell(rand.Float64())
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())
	start = time.Now()

	for i := 0; i < nCells; i++ {
		res[i] = one[i] * two[i]
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapIntegerIndexSmallCellsValue(nCells int) {
	start := time.Now()
	variableSize := ((8 + 32) * float64(nCells)) / math.Pow(2, 20)
	fmt.Printf("\tmap[int64]struct{padding [24]byte,value float64}, ~%.1f MiB\n", variableSize)

	one := make(map[int]SmallCell, nCells)
	two := make(map[int]SmallCell, nCells)
	res := make(map[int]SmallCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = SmallCell{value: rand.Float64()}
		two[i] = SmallCell{value: rand.Float64()}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i] = SmallCell{value: one[i].value * two[i].value}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapIntegerIndexSmallCells(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[int64]*struct{padding [24]byte,value float64}, ~%.1f MiB\n", ((8+8+32)*float64(nCells))/math.Pow(2, 20))

	one := make(map[int]*SmallCell, nCells)
	two := make(map[int]*SmallCell, nCells)
	res := make(map[int]*SmallCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = &SmallCell{value: rand.Float64()}
		two[i] = &SmallCell{value: rand.Float64()}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i] = &SmallCell{value: one[i].value * two[i].value}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapIntegerIndexSmallerCells(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[int64]*struct{padding [56]byte,value float64}, ~%.1f MiB\n", ((8+8+56)*float64(nCells))/math.Pow(2, 20))

	one := make(map[int]*SmallerCell, nCells)
	two := make(map[int]*SmallerCell, nCells)
	res := make(map[int]*SmallerCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = &SmallerCell{value: rand.Float64()}
		two[i] = &SmallerCell{value: rand.Float64()}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i] = &SmallerCell{value: one[i].value * two[i].value}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapIntegerIndex(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[int64]*struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+8+80)*float64(nCells))/math.Pow(2, 20))

	one := make(map[int]*LargeCell, nCells)
	two := make(map[int]*LargeCell, nCells)
	res := make(map[int]*LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = &LargeCell{value: rand.Float64()}
		two[i] = &LargeCell{value: rand.Float64()}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i] = &LargeCell{value: one[i].value * two[i].value}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapSmallCells(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[struct{timespan uint8,rank uint64}]*struct{padding [24]byte,value float64}, ~%.1f MiB\n", ((8+8+64)*float64(nCells))/math.Pow(2, 20))

	one := make(map[Rank]*SmallCell, nCells)
	two := make(map[Rank]*SmallCell, nCells)
	res := make(map[Rank]*SmallCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			one[rank] = &SmallCell{value: rand.Float64()}
			two[rank] = &SmallCell{value: rand.Float64()}
		}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			res[rank] = &SmallCell{value: one[rank].value * two[rank].value}
		}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapSmallerCells(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[struct{timespan uint8,rank uint64}]*struct{padding [56]byte,value float64}, ~%.1f MiB\n", ((8+8+64)*float64(nCells))/math.Pow(2, 20))

	one := make(map[Rank]*SmallerCell, nCells)
	two := make(map[Rank]*SmallerCell, nCells)
	res := make(map[Rank]*SmallerCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			one[rank] = &SmallerCell{value: rand.Float64()}
			two[rank] = &SmallerCell{value: rand.Float64()}
		}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			res[rank] = &SmallerCell{value: one[rank].value * two[rank].value}
		}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellValuesSmallest(nCells int) {
	start := time.Now()
	fmt.Printf("\t[]struct{padding [8]byte,value float64}, ~%.1f MiB\n", ((16)*float64(nCells))/math.Pow(2, 20))

	one := make([]SmallestCell, nCells)
	two := make([]SmallestCell, nCells)
	res := make([]SmallestCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i].value = rand.Float64()
		two[i].value = rand.Float64()
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i].value = one[i].value * two[i].value
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellValuesSmall(nCells int) {
	start := time.Now()
	fmt.Printf("\t[]struct{padding [24]byte,value float64}, ~%.1f MiB\n", ((32)*float64(nCells))/math.Pow(2, 20))

	one := make([]SmallCell, nCells)
	two := make([]SmallCell, nCells)
	res := make([]SmallCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i].value = rand.Float64()
		two[i].value = rand.Float64()
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i].value = one[i].value * two[i].value
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellValuesSmaller(nCells int) {
	start := time.Now()
	fmt.Printf("\t[]struct{padding [56]byte,value float64}, ~%.1f MiB\n", ((64)*float64(nCells))/math.Pow(2, 20))

	one := make([]SmallerCell, nCells)
	two := make([]SmallerCell, nCells)
	res := make([]SmallerCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i].value = rand.Float64()
		two[i].value = rand.Float64()
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i].value = one[i].value * two[i].value
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellValues(nCells int) {
	start := time.Now()
	fmt.Printf("\t[]struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((88)*float64(nCells))/math.Pow(2, 20))

	one := make([]LargeCell, nCells)
	two := make([]LargeCell, nCells)
	res := make([]LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i].value = rand.Float64()
		two[i].value = rand.Float64()
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i].value = one[i].value * two[i].value
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellPointersPreAllocated(nCells int) {
	start := time.Now()
	fmt.Printf("\tARENA ALLOCATION\n")
	fmt.Printf("\t[]*struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+88)*float64(nCells))/math.Pow(2, 20))

	one := make([]*LargeCell, nCells)
	two := make([]*LargeCell, nCells)
	res := make([]*LargeCell, nCells)

	oneCells := make([]LargeCell, nCells)
	twoCells := make([]LargeCell, nCells)
	threeCells := make([]LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		oneCells[i].value = rand.Float64()
		twoCells[i].value = rand.Float64()
		one[i] = &oneCells[i]
		two[i] = &twoCells[i]
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		threeCells[i].value = one[i].value * two[i].value
		res[i] = &threeCells[i]
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func arrayCellPointers(nCells int) {
	start := time.Now()
	fmt.Printf("\t[]*struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+88)*float64(nCells))/math.Pow(2, 20))

	one := make([]*LargeCell, nCells)
	two := make([]*LargeCell, nCells)
	res := make([]*LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := 0; i < nCells; i++ {
		one[i] = &LargeCell{value: rand.Float64()}
		two[i] = &LargeCell{value: rand.Float64()}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := 0; i < nCells; i++ {
		res[i] = &LargeCell{value: one[i].value * two[i].value}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMapPreAllocate(nCells int) {
	start := time.Now()
	fmt.Printf("\tARENA ALLOCATION\n")
	fmt.Printf("\tmap[struct{timespan uint8,rank uint64}]*struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+8+88)*float64(nCells))/math.Pow(2, 20))

	oneCells := make([]LargeCell, nCells)
	twoCells := make([]LargeCell, nCells)
	threeCells := make([]LargeCell, nCells)

	one := make(map[Rank]*LargeCell, nCells)
	two := make(map[Rank]*LargeCell, nCells)
	res := make(map[Rank]*LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	k := 0
	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			oneCells[i].value = rand.Float64()
			twoCells[i].value = rand.Float64()
			one[rank] = &oneCells[i]
			two[rank] = &twoCells[i]
			k += 1
		}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	k = 0
	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			k += 1
			rank := Rank{i, j}
			threeCells[k].value = one[rank].value * two[rank].value
			res[rank] = &threeCells[k]
		}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func pointerMap(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[struct{timespan uint8,rank uint64}]*struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+8+88)*float64(nCells))/math.Pow(2, 20))

	one := make(map[Rank]*LargeCell, nCells)
	two := make(map[Rank]*LargeCell, nCells)
	res := make(map[Rank]*LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			one[rank] = &LargeCell{value: rand.Float64()}
			two[rank] = &LargeCell{value: rand.Float64()}
		}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			res[rank] = &LargeCell{value: one[rank].value * two[rank].value}
		}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

func valueMap(nCells int) {
	start := time.Now()
	fmt.Printf("\tmap[struct{timespan uint8,rank uint64}]struct{padding [80]byte,value float64}, ~%.1f MiB\n", ((8+88)*float64(nCells))/math.Pow(2, 20))

	one := make(map[Rank]LargeCell, nCells)
	two := make(map[Rank]LargeCell, nCells)
	res := make(map[Rank]LargeCell, nCells)

	rand := rand.New(rand.NewSource(0xCA0541))

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			one[rank] = LargeCell{value: rand.Float64()}
			two[rank] = LargeCell{value: rand.Float64()}
		}
	}

	fmt.Printf("\t\tNode generation %d ms\n", time.Now().Sub(start).Milliseconds())

	for i := uint8(0); i < 255; i++ {
		for j := int64(0); j < int64(nCells/255); j++ {
			rank := Rank{i, j}
			res[rank] = LargeCell{value: one[rank].value * two[rank].value}
		}
	}
	fmt.Printf("\t\tMultiplication %d ms\n\n", time.Now().Sub(start).Milliseconds())
}

package util

import (
	"sync"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestOnceValue(t *testing.T) {
	t.Run("sets value only once", func(t *testing.T) {
		once := &OnceValue[int]{}

		once.Set(42)
		require.Equal(t, 42, once.V, "first set should store the value")

		once.Set(84)
		require.Equal(t, 42, once.V, "second set should not change the value")
	})

	t.Run("concurrent access", func(t *testing.T) {
		once := &OnceValue[string]{}
		var wg sync.WaitGroup
		numGoroutines := 10

		wg.Add(numGoroutines)
		for i := 0; i < numGoroutines; i++ {
			go func() {
				defer wg.Done()
				once.Set("test")
			}()
		}
		wg.Wait()

		require.Equal(t, "test", once.V, "value should be set correctly under concurrent access")
	})

	t.Run("zero value initialization", func(t *testing.T) {
		var once OnceValue[int]
		once.Set(1)
		require.Equal(t, 1, once.V, "should work with zero-value initialized struct")
	})

	t.Run("with complex type", func(t *testing.T) {
		type complex struct {
			field string
		}

		once := &OnceValue[complex]{}
		value := complex{field: "hello"}
		once.Set(value)

		require.Equal(t, "hello", once.V.field, "should store complex type correctly")

		once.Set(complex{field: "world"})
		require.Equal(t, "hello", once.V.field, "should not change value on second set")
	})
}

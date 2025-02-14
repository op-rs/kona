package util

import "sync"

type OnceValue[T any] struct {
	V    T
	once sync.Once
}

func (o *OnceValue[T]) Set(value T) {
	o.once.Do(func() {
		o.V = value
	})
}

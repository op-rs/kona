package config

import (
	"fmt"
	"reflect"
	"time"
)

type HardforkTime uint64

func NewHardforkTime(t uint64) *HardforkTime {
	h := HardforkTime(t)
	return &h
}

func (h HardforkTime) MarshalTOML() ([]byte, error) {
	ts := time.Unix(int64(h), 0).UTC().Format("Mon 2 Jan 2006 15:04:05 MST")
	return []byte(fmt.Sprintf(`%d # %s`, uint64(h), ts)), nil
}

func (h *HardforkTime) U64Ptr() *uint64 {
	if h == nil {
		return nil
	}

	return (*uint64)(h)
}

func CopyHardforks(src, dst *Hardforks, superchainTime, genesisTime *uint64) error {
	if superchainTime == nil {
		// No changes if SuperchainTime is unset
		return nil
	}

	if genesisTime == nil {
		return fmt.Errorf("genesisTime is nil")
	}

	srcVal := reflect.ValueOf(src).Elem()
	dstVal := reflect.ValueOf(dst).Elem()

	// Iterate through all fields in the source struct
	for i := 0; i < srcVal.NumField(); i++ {
		srcField := srcVal.Field(i)

		if srcField.IsNil() {
			continue
		}

		// Get the corresponding field in the destination
		dstField := dstVal.Field(i)

		// Only copy if dest is nil
		if !dstField.IsNil() {
			continue
		}

		srcValue := reflect.Indirect(srcField).Uint()
		if srcValue < *superchainTime {
			// No change if hardfork activated before SuperchainTime
			continue
		}

		if srcValue > *genesisTime {
			// Use src value if is after genesis
			dstField.Set(srcField)
		} else {
			// Use zero if it is equal to or before genesis
			ptrZero := reflect.ValueOf(NewHardforkTime(0))
			dstField.Set(ptrZero)
		}
	}

	return nil
}

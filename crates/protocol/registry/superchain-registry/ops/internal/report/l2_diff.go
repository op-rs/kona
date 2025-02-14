package report

import (
	"bytes"
	"maps"
	"slices"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
)

func DiffAllocs(a types.GenesisAlloc, b types.GenesisAlloc) []AccountDiff {
	bCopy := maps.Clone(b)

	out := make([]AccountDiff, 0)

	for addrA, accA := range a {
		accB, ok := bCopy[addrA]
		if !ok {
			diff := AccountDiff{
				Address: addrA,
				Removed: true,

				CodeChanged:    true,
				BalanceChanged: true,
				NonceChanged:   true,

				OldCode:    accA.Code,
				OldBalance: accA.Balance,
				OldNonce:   accA.Nonce,

				StorageChanges: computeStorageDiff(accA.Storage, nil),
			}
			out = append(out, diff)
			continue
		}

		diff := AccountDiff{
			Address:        addrA,
			CodeChanged:    !bytes.Equal(accA.Code, accB.Code),
			BalanceChanged: accA.Balance.Cmp(accB.Balance) != 0,
			NonceChanged:   accA.Nonce != accB.Nonce,
			StorageChanges: computeStorageDiff(accA.Storage, accB.Storage),
		}

		if diff.CodeChanged {
			diff.OldCode = accA.Code
			diff.NewCode = accB.Code
		}

		if diff.BalanceChanged {
			diff.OldBalance = accA.Balance
			diff.NewBalance = accB.Balance
		}

		if diff.NonceChanged {
			diff.OldNonce = accA.Nonce
			diff.NewNonce = accB.Nonce
		}

		if diff.CodeChanged || diff.BalanceChanged || diff.NonceChanged || len(diff.StorageChanges) > 0 {
			out = append(out, diff)
		}

		delete(bCopy, addrA)
	}

	for addrB, accB := range bCopy {
		diff := AccountDiff{
			Address: addrB,
			Added:   true,

			CodeChanged:    true,
			BalanceChanged: true,
			NonceChanged:   true,

			NewCode:        accB.Code,
			NewBalance:     accB.Balance,
			NewNonce:       accB.Nonce,
			StorageChanges: computeStorageDiff(nil, accB.Storage),
		}

		out = append(out, diff)
	}

	return out
}

func computeStorageDiff(a map[common.Hash]common.Hash, b map[common.Hash]common.Hash) []StorageDiff {
	out := make([]StorageDiff, 0)

	bCopy := maps.Clone(b)

	for keyA, valueA := range a {
		valueB, ok := bCopy[keyA]
		if !ok {
			diff := StorageDiff{
				Key:      keyA,
				Removed:  true,
				OldValue: valueA,
			}
			out = append(out, diff)
			continue
		}

		if valueA != valueB {
			diff := StorageDiff{
				Key:      keyA,
				OldValue: valueA,
				NewValue: valueB,
			}
			out = append(out, diff)
		}

		delete(bCopy, keyA)
	}

	for keyB, valueB := range bCopy {
		diff := StorageDiff{
			Key:      keyB,
			Added:    true,
			NewValue: valueB,
		}
		out = append(out, diff)
	}

	slices.SortFunc(out, func(a, b StorageDiff) int {
		return bytes.Compare(a.Key.Bytes(), b.Key.Bytes())
	})

	if len(out) == 0 {
		return nil
	}

	return out
}

package report

import (
	"math/big"
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/stretchr/testify/require"
)

func TestDiffAllocs(t *testing.T) {
	addr1 := common.HexToAddress("0x1")
	addr2 := common.HexToAddress("0x2")
	addr3 := common.HexToAddress("0x3")

	key1 := common.HexToHash("0x1")
	key2 := common.HexToHash("0x2")
	key3 := common.HexToHash("0x3")
	val1 := common.HexToHash("0xa")
	val2 := common.HexToHash("0xb")
	val3 := common.HexToHash("0xc")

	tests := []struct {
		name     string
		allocA   types.GenesisAlloc
		allocB   types.GenesisAlloc
		expected []AccountDiff
	}{
		{
			name: "identical allocs",
			allocA: types.GenesisAlloc{
				addr1: {
					Code:    []byte{1, 2, 3},
					Balance: big.NewInt(100),
					Nonce:   1,
					Storage: map[common.Hash]common.Hash{
						key1: val1,
						key2: val2,
					},
				},
			},
			allocB: types.GenesisAlloc{
				addr1: {
					Code:    []byte{1, 2, 3},
					Balance: big.NewInt(100),
					Nonce:   1,
					Storage: map[common.Hash]common.Hash{
						key1: val1,
						key2: val2,
					},
				},
			},
			expected: []AccountDiff{},
		},
		{
			name: "account modifications",
			allocA: types.GenesisAlloc{
				addr1: {
					Code:    []byte{1, 2, 3},
					Balance: big.NewInt(100),
					Nonce:   1,
					Storage: map[common.Hash]common.Hash{
						key1: val1,
						key2: val2,
					},
				},
			},
			allocB: types.GenesisAlloc{
				addr1: {
					Code:    []byte{1, 2, 4},
					Balance: big.NewInt(150),
					Nonce:   2,
					Storage: map[common.Hash]common.Hash{
						key1: val1,
						key2: val3,
					},
				},
			},
			expected: []AccountDiff{
				{
					Address:        addr1,
					CodeChanged:    true,
					BalanceChanged: true,
					NonceChanged:   true,
					OldCode:        []byte{1, 2, 3},
					NewCode:        []byte{1, 2, 4},
					OldBalance:     big.NewInt(100),
					NewBalance:     big.NewInt(150),
					OldNonce:       1,
					NewNonce:       2,
					StorageChanges: []StorageDiff{
						{
							Key:      key2,
							OldValue: val2,
							NewValue: val3,
						},
					},
				},
			},
		},
		{
			name: "account removal",
			allocA: types.GenesisAlloc{
				addr2: {
					Code:    []byte{1},
					Balance: big.NewInt(100),
					Nonce:   1,
					Storage: map[common.Hash]common.Hash{
						key1: val1,
					},
				},
			},
			allocB: types.GenesisAlloc{},
			expected: []AccountDiff{
				{
					Address:        addr2,
					Removed:        true,
					CodeChanged:    true,
					BalanceChanged: true,
					NonceChanged:   true,
					OldCode:        []byte{1},
					OldBalance:     big.NewInt(100),
					OldNonce:       1,
					StorageChanges: []StorageDiff{
						{
							Key:      key1,
							Removed:  true,
							OldValue: val1,
						},
					},
				},
			},
		},
		{
			name:   "account addition",
			allocA: types.GenesisAlloc{},
			allocB: types.GenesisAlloc{
				addr3: {
					Code:    []byte{1},
					Balance: big.NewInt(100),
					Nonce:   1,
					Storage: map[common.Hash]common.Hash{
						key3: val3,
					},
				},
			},
			expected: []AccountDiff{
				{
					Address:        addr3,
					Added:          true,
					CodeChanged:    true,
					BalanceChanged: true,
					NonceChanged:   true,
					NewCode:        []byte{1},
					NewBalance:     big.NewInt(100),
					NewNonce:       1,
					StorageChanges: []StorageDiff{
						{
							Key:      key3,
							Added:    true,
							NewValue: val3,
						},
					},
				},
			},
		},
		{
			name: "storage modifications",
			allocA: types.GenesisAlloc{
				addr1: {
					Storage: map[common.Hash]common.Hash{
						key1: val1,
						key2: val2,
						key3: val3,
					},
				},
			},
			allocB: types.GenesisAlloc{
				addr1: {
					Storage: map[common.Hash]common.Hash{
						key1: val2,
						key3: val3,
						key2: val1,
					},
				},
			},
			expected: []AccountDiff{
				{
					Address: addr1,
					StorageChanges: []StorageDiff{
						{
							Key:      key1,
							OldValue: val1,
							NewValue: val2,
						},
						{
							Key:      key2,
							OldValue: val2,
							NewValue: val1,
						},
					},
				},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			diffs := DiffAllocs(tt.allocA, tt.allocB)
			require.Equal(t, tt.expected, diffs)
		})
	}
}

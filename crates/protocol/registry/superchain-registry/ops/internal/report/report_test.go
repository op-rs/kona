package report

import (
	"math/big"
	"strings"
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/stretchr/testify/require"
)

func TestAccountDiff_AsMarkdown(t *testing.T) {
	tests := []struct {
		name string
		diff AccountDiff
		want string
	}{
		{
			name: "added account",
			diff: AccountDiff{
				Address:    common.HexToAddress("0x123"),
				Added:      true,
				NewCode:    []byte{1, 2, 3},
				NewBalance: big.NewInt(100),
				NewNonce:   1,
				StorageChanges: []StorageDiff{
					{
						Key:      common.HexToHash("0x456"),
						NewValue: common.HexToHash("0x789"),
					},
				},
			},
			want: `
+0x0000000000000000000000000000000000000123
+code:0x010203
+balance:100
+nonce:1
+storage:
+  0x0000000000000000000000000000000000000000000000000000000000000456:0x0000000000000000000000000000000000000000000000000000000000000789
`,
		},
		{
			name: "removed account",
			diff: AccountDiff{
				Address:    common.HexToAddress("0x123"),
				Removed:    true,
				OldCode:    []byte{1, 2, 3},
				OldBalance: big.NewInt(100),
				OldNonce:   1,
				StorageChanges: []StorageDiff{
					{
						Key:      common.HexToHash("0x456"),
						OldValue: common.HexToHash("0x789"),
					},
				},
			},
			want: `
-0x0000000000000000000000000000000000000123
-code:0x010203
-balance:100
-nonce:1
-storage:
-  0x0000000000000000000000000000000000000000000000000000000000000456:0x0000000000000000000000000000000000000000000000000000000000000789
`,
		},
		{
			name: "modified account",
			diff: AccountDiff{
				Address:        common.HexToAddress("0x123"),
				CodeChanged:    true,
				BalanceChanged: true,
				NonceChanged:   true,
				OldCode:        []byte{1, 2, 3},
				NewCode:        []byte{4, 5, 6},
				OldBalance:     big.NewInt(100),
				NewBalance:     big.NewInt(200),
				OldNonce:       1,
				NewNonce:       2,
				StorageChanges: []StorageDiff{
					{
						Key:      common.HexToHash("0x456"),
						OldValue: common.HexToHash("0x789"),
						NewValue: common.HexToHash("0xabc"),
					},
					{
						Key:      common.HexToHash("0x789"),
						Removed:  true,
						OldValue: common.HexToHash("0x789"),
					},
					{
						Key:      common.HexToHash("0xdef"),
						Added:    true,
						NewValue: common.HexToHash("0x123"),
					},
				},
			},
			want: `
0x0000000000000000000000000000000000000123
-code:0x010203
+code:0x040506
-balance:100
+balance:200
-nonce:1
+nonce:2
storage:
-  0x0000000000000000000000000000000000000000000000000000000000000456:0x0000000000000000000000000000000000000000000000000000000000000789
+  0x0000000000000000000000000000000000000000000000000000000000000456:0x0000000000000000000000000000000000000000000000000000000000000abc
-  0x0000000000000000000000000000000000000000000000000000000000000789:0x0000000000000000000000000000000000000000000000000000000000000789
+  0x0000000000000000000000000000000000000000000000000000000000000def:0x0000000000000000000000000000000000000000000000000000000000000123
`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.diff.AsMarkdown()
			require.Equal(t, strings.TrimSpace(tt.want), strings.TrimSpace(got))
		})
	}
}

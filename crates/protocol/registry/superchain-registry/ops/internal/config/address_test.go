package config

import (
	"fmt"
	"strings"
	"testing"

	"github.com/BurntSushi/toml"
	"github.com/ethereum/go-ethereum/common"
	"github.com/stretchr/testify/require"
)

func TestChecksummedAddress_MarshalTOML(t *testing.T) {
	addr := common.HexToAddress(strings.ToLower("0xdE1FCfB0851916CA5101820A69b13a4E276bd81F"))
	checksummedAddr := ChecksummedAddress(addr)
	out, err := toml.Marshal(checksummedAddr)
	require.NoError(t, err)
	require.Equal(t, `"0xdE1FCfB0851916CA5101820A69b13a4E276bd81F"`, string(out))
}

func TestChecksummedAddress_UnmarshalTOML(t *testing.T) {
	tests := []struct {
		name    string
		in      string
		want    ChecksummedAddress
		wantErr string
	}{
		{
			name: "valid",
			in:   `"0xdE1FCfB0851916CA5101820A69b13a4E276bd81F"`,
			want: ChecksummedAddress(common.HexToAddress("0xdE1FCfB0851916CA5101820A69b13a4E276bd81F")),
		},
		{
			name:    "invalid",
			in:      strings.ToLower(`"0xdE1FCfB0851916CA5101820A69b13a4E276bd81F"`),
			wantErr: "invalid checksum",
		},
		{
			name:    "invalid - empty",
			in:      ``,
			wantErr: "unexpected EOF",
		},
		{
			name:    "invalid - not (high))",
			in:      `0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF`,
			wantErr: "out of range",
		},
		{
			name:    "invalid - not quoted (low)",
			in:      `0x0000000000000000000000000000000000000001`,
			wantErr: "expected a string, got int64",
		},
		{
			name:    "invalid - no hex data",
			in:      `""`,
			wantErr: "invalid address",
		},
		{
			name:    "invalid - bad length",
			in:      `"0xdE1FCfB08"`,
			wantErr: "invalid address",
		},
		{
			name:    "invalid - not hex",
			in:      `"falafels"`,
			wantErr: "invalid address",
		},
	}
	type addrHolder struct {
		Addr ChecksummedAddress `toml:"addr"`
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var ah addrHolder
			err := toml.Unmarshal([]byte(fmt.Sprintf(`addr = %s`, tt.in)), &ah)
			if tt.wantErr != "" {
				require.Error(t, err)
				require.Contains(t, err.Error(), tt.wantErr)
				return
			}
			require.NoError(t, err)
			require.Equal(t, tt.want, ah.Addr)
		})
	}
}

package validation

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestAddress(t *testing.T) {
	t.Run("valid address marshal/unmarshal", func(t *testing.T) {
		addr := Address{}
		validAddrStr := "0x1234567890123456789012345678901234567890"

		err := addr.UnmarshalText([]byte(validAddrStr))
		require.NoError(t, err)

		marshaled, err := addr.MarshalText()
		require.NoError(t, err)
		require.Equal(t, validAddrStr, string(marshaled))
		require.Equal(t, validAddrStr, addr.String())
	})

	t.Run("invalid address length", func(t *testing.T) {
		addr := Address{}
		invalidAddr := "0x123" // too short
		err := addr.UnmarshalText([]byte(invalidAddr))
		require.Error(t, err)
		require.Contains(t, err.Error(), "invalid address length")
	})

	t.Run("invalid prefix", func(t *testing.T) {
		addr := Address{}
		invalidAddr := "1x1234567890123456789012345678901234567890"
		err := addr.UnmarshalText([]byte(invalidAddr))
		require.Error(t, err)
		require.Contains(t, err.Error(), "invalid address prefix")
	})

	t.Run("invalid hex characters", func(t *testing.T) {
		addr := Address{}
		invalidAddr := "0xg234567890123456789012345678901234567890"
		err := addr.UnmarshalText([]byte(invalidAddr))
		require.Error(t, err)
	})
}

func TestHash(t *testing.T) {
	t.Run("valid hash marshal/unmarshal", func(t *testing.T) {
		hash := Hash{}
		validHashStr := "0x1234567890123456789012345678901234567890123456789012345678901234"

		err := hash.UnmarshalText([]byte(validHashStr))
		require.NoError(t, err)

		marshaled, err := hash.MarshalText()
		require.NoError(t, err)
		require.Equal(t, validHashStr, string(marshaled))
		require.Equal(t, validHashStr, hash.String())
	})

	t.Run("empty hash", func(t *testing.T) {
		hash := Hash{}
		err := hash.UnmarshalText([]byte{})
		require.NoError(t, err)
	})

	t.Run("missing 0x prefix", func(t *testing.T) {
		hash := Hash{}
		invalidHash := "1234567890123456789012345678901234567890123456789012345678901234"
		err := hash.UnmarshalText([]byte(invalidHash))
		require.Error(t, err)
		require.Contains(t, err.Error(), "hex string must have 0x prefix")
	})

	t.Run("invalid hash length", func(t *testing.T) {
		hash := Hash{}
		invalidHash := "0x123456" // too short
		err := hash.UnmarshalText([]byte(invalidHash))
		require.Error(t, err)
		require.Contains(t, err.Error(), "invalid hash length")
	})

	t.Run("invalid hex characters", func(t *testing.T) {
		hash := Hash{}
		invalidHash := "0x123g567890123456789012345678901234567890123456789012345678901234"
		err := hash.UnmarshalText([]byte(invalidHash))
		require.Error(t, err)
		require.Contains(t, err.Error(), "invalid hex string")
	})
}

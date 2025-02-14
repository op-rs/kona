package validation

import (
	"bytes"
	"encoding/hex"
	"fmt"
)

type Address [20]byte

func (a *Address) UnmarshalText(text []byte) error {
	if len(text) != 42 {
		return fmt.Errorf("invalid address length: %d", len(text))
	}
	if text[0] != '0' || text[1] != 'x' {
		return fmt.Errorf("invalid address prefix: %s", text)
	}

	addr, err := hex.DecodeString(string(text[2:]))
	if err != nil {
		return err
	}
	copy(a[:], addr)
	return nil
}

func (a Address) MarshalText() ([]byte, error) {
	return []byte(a.String()), nil
}

func (a Address) String() string {
	return "0x" + hex.EncodeToString(a[:])
}

type Hash [32]byte

func (h Hash) String() string {
	return "0x" + hex.EncodeToString(h[:])
}

func (h Hash) MarshalText() ([]byte, error) {
	return []byte(h.String()), nil
}

func (h *Hash) UnmarshalText(text []byte) error {
	if len(text) == 0 {
		return nil
	}

	if len(text) < 2 || !bytes.HasPrefix(text, []byte("0x")) {
		return fmt.Errorf("hex string must have 0x prefix")
	}

	b, err := hex.DecodeString(string(text[2:]))
	if err != nil {
		return fmt.Errorf("invalid hex string: %w", err)
	}

	if len(b) != len(h) {
		return fmt.Errorf("invalid hash length: got %d, want %d", len(b), len(h))
	}

	copy(h[:], b)
	return nil
}

package config

import (
	"testing"

	"github.com/BurntSushi/toml"
	"github.com/stretchr/testify/require"
)

func TestHardforkTime_MarshalTOML(t *testing.T) {
	type hftHolder struct {
		HardforkTime *HardforkTime `toml:"time,omitempty"`
	}

	tests := []struct {
		name string
		hft  hftHolder
		want string
	}{
		{
			name: "nil",
			hft:  hftHolder{},
			want: "",
		},
		{
			name: "zero",
			hft:  hftHolder{HardforkTime: new(HardforkTime)},
			want: "time = 0 # Thu 1 Jan 1970 00:00:00 UTC\n",
		},
		{
			name: "non-zero",
			hft:  hftHolder{HardforkTime: NewHardforkTime(1708560000)},
			want: "time = 1708560000 # Thu 22 Feb 2024 00:00:00 UTC\n",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := toml.Marshal(tt.hft)
			require.NoError(t, err)
			require.Equal(t, tt.want, string(got))
		})
	}
}

func TestCopyHardforks(t *testing.T) {
	src := &Hardforks{ // e.g. in superchain.toml
		CanyonTime: NewHardforkTime(1),
		DeltaTime:  NewHardforkTime(2),
	}
	dest := &Hardforks{ // e.g in individual chain config
		CanyonTime: NewHardforkTime(0),
	}

	two := uint64(2)
	zero := uint64(0)

	require.NoError(t,
		CopyHardforks(
			src,
			dest,
			&two,  // superchainTime
			&zero, // genesisTime
		))

	require.Equal(t, &Hardforks{
		CanyonTime: NewHardforkTime(0),
		DeltaTime:  NewHardforkTime(2),
	}, dest)
}

func TestCopyHardforksActivationSemantics(t *testing.T) {
	canyonAt := func(t uint64) *Hardforks {
		return &Hardforks{
			CanyonTime: NewHardforkTime(t),
		}
	}
	nilCanyon := func() *Hardforks {
		return &Hardforks{
			CanyonTime: nil,
		}
	}

	genesisAt := func(t uint64) *uint64 {
		h := uint64(t)
		return &h
	}
	superchainTimeAt := genesisAt

	type testCase struct {
		name           string
		src            *Hardforks
		dest           *Hardforks
		superchainTime *uint64
		genesisTime    *uint64
		expectedDest   *Hardforks
	}

	testCases := []testCase{
		{"src+dest(nil_superchain_time)=dest", canyonAt(8), canyonAt(3), nil, genesisAt(0), canyonAt(3)},
		{"src(nil)+dest=dest", nilCanyon(), canyonAt(8), nil, genesisAt(0), canyonAt(8)},
		{"src(nil,nil_superchain_time)+dest=dest", nilCanyon(), canyonAt(8), superchainTimeAt(0), genesisAt(0), canyonAt(8)},
		{"src(after_superchain_time)+dest(nil)=src", canyonAt(3), nilCanyon(), superchainTimeAt(2), genesisAt(0), canyonAt(3)},
		{"src(after_zero_superchain_time)+dest(nil)=src", canyonAt(3), nilCanyon(), superchainTimeAt(0), genesisAt(0), canyonAt(3)},
		{"src(before_superchain_time)+dest(nil)=dest", canyonAt(3), nilCanyon(), superchainTimeAt(10), genesisAt(0), nilCanyon()},
		{"src(after_superchain_time_before_genesis)+dest(nil)=0", canyonAt(3), nilCanyon(), superchainTimeAt(1), genesisAt(4), canyonAt(0)},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			require.NoError(t,
				CopyHardforks(
					tc.src,
					tc.dest,
					tc.superchainTime,
					tc.genesisTime,
				))
			require.Equal(t, tc.expectedDest, tc.dest)
		})
	}
}

package output

import (
	"fmt"
	"os"
)

func WriteStderr(in string, args ...any) {
	fmt.Fprintf(os.Stderr, in+"\n", args...)
}

func WriteOK(in string, args ...any) {
	WriteStderr("[   OK] "+in, args...)
}

func WriteNotOK(in string, args ...any) {
	WriteStderr("[NOTOK] "+in, args...)
}

func WriteWarn(in string, args ...any) {
	WriteStderr("[ WARN] "+in, args...)
}

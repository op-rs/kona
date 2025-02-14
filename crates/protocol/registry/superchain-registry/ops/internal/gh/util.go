package gh

import (
	"fmt"
	"strconv"
	"strings"
)

func GetPRNumberFromURL(url string) (int, error) {
	prNumSplit := strings.Split(url, "/")
	prNumStr := prNumSplit[len(prNumSplit)-1]
	prNum, err := strconv.Atoi(prNumStr)
	if err != nil {
		return 0, fmt.Errorf("failed to parse PR number: %w", err)
	}
	return prNum, nil
}

func SplitOrgRepo(repo string) (string, string) {
	parts := strings.Split(repo, "/")
	if len(parts) != 2 {
		return "", ""
	}
	return parts[0], parts[1]
}

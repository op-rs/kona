package gh

import (
	"context"
	"fmt"
	"strings"

	"github.com/google/go-github/v68/github"
)

type FindCommentPred func(comment *github.IssueComment) bool

type Commenter interface {
	PostComment(ctx context.Context, pr int, message string) error
	EditComment(ctx context.Context, commentID int64, message string) error
	FindComment(ctx context.Context, pr int, preds ...FindCommentPred) (*github.IssueComment, error)
}

type GithubCommenter struct {
	owner    string
	repo     string
	ghClient *github.Client
}

func NewGithubCommenter(repoFull string, ghClient *github.Client) *GithubCommenter {
	owner, repoName := SplitOrgRepo(repoFull)
	return &GithubCommenter{
		owner:    owner,
		repo:     repoName,
		ghClient: ghClient,
	}
}

func (gc *GithubCommenter) PostComment(ctx context.Context, pr int, message string) error {
	comment := &github.IssueComment{
		Body: &message,
	}

	_, _, err := gc.ghClient.Issues.CreateComment(
		ctx,
		gc.owner,
		gc.repo,
		pr,
		comment,
	)
	if err != nil {
		return fmt.Errorf("failed to post comment: %w", err)
	}

	return nil
}

func (gc *GithubCommenter) EditComment(ctx context.Context, commentID int64, message string) error {
	comment := &github.IssueComment{
		Body: &message,
	}

	_, _, err := gc.ghClient.Issues.EditComment(
		ctx,
		gc.owner,
		gc.repo,
		commentID,
		comment,
	)
	if err != nil {
		return fmt.Errorf("failed to edit comment: %w", err)
	}

	return nil
}

func (gc *GithubCommenter) FindComment(ctx context.Context, pr int, preds ...FindCommentPred) (*github.IssueComment, error) {
	opts := &github.IssueListCommentsOptions{
		ListOptions: github.ListOptions{
			PerPage: 100,
		},
	}

	for {
		comments, resp, err := gc.ghClient.Issues.ListComments(
			ctx,
			gc.owner,
			gc.repo,
			pr,
			opts,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to list comments: %w", err)
		}

		for _, comment := range comments {
			for _, pred := range preds {
				if pred(comment) {
					return comment, nil
				}
			}
		}

		if resp.NextPage == 0 {
			break
		}
		opts.Page = resp.NextPage
	}

	return nil, nil
}

func CommentContaining(substring string) FindCommentPred {
	return func(comment *github.IssueComment) bool {
		return strings.Contains(comment.GetBody(), substring)
	}
}

func CommentFrom(user string) FindCommentPred {
	return func(comment *github.IssueComment) bool {
		return comment.User.GetLogin() == user
	}
}

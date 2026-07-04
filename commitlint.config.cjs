/**
 * Commitlint configuration — enforces Conventional Commits.
 *
 * Commit types drive release-please SemVer bumps:
 *   fix:      -> patch   feat:  -> minor   feat!/BREAKING CHANGE -> major
 *   chore/docs/refactor/test/ci/build/perf/style/revert -> no release bump
 *
 * See docs/VERSIONING.md and CONTRIBUTING.md.
 */
module.exports = {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "chore",
        "docs",
        "refactor",
        "test",
        "ci",
        "build",
        "perf",
        "style",
        "revert",
      ],
    ],
    "body-max-line-length": [0, "always", Infinity],
  },
};

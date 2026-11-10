"""Checks for the release version rule: `python3 -I -m unittest discover -s scripts/tests`."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import release_version  # noqa: E402
from release_version import Release, newest_tag, next_release  # noqa: E402

SCRIPT = Path(release_version.__file__)


class FirstRelease(unittest.TestCase):
    def test_uses_the_workspace_version_whatever_the_commits(self):
        for messages in ([], ["docs: only words"], ["chore: tidy", "Update readme"]):
            with self.subTest(messages=messages):
                self.assertEqual(next_release(None, messages, "0.1.0"), Release("0.1.0", True))

    def test_follows_the_workspace_version(self):
        self.assertEqual(next_release(None, [], "0.3.0"), Release("0.3.0", True))

    def test_rejects_a_workspace_version_that_is_not_plain(self):
        for version in ("0.1.0-dev", "0.1", "01.1.0", "", "1.0.0+build"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                next_release(None, [], version)


class Bump(unittest.TestCase):
    def release(self, *messages, tag="v1.2.3"):
        return next_release(tag, list(messages), "0.1.0")

    def test_feat_bumps_minor(self):
        self.assertEqual(self.release("feat(net): add a sampler (#12)\n\n* detail"), Release("1.3.0", True))

    def test_fix_perf_and_revert_bump_patch(self):
        for message in ("fix: stop a crash", "perf(engine): cache the grid (#3)", "revert: undo the cache"):
            with self.subTest(message=message):
                self.assertEqual(self.release(message), Release("1.2.4", True))

    def test_other_types_release_nothing(self):
        for kind in ("docs", "ci", "chore", "test", "refactor", "build", "style", "wat"):
            with self.subTest(kind=kind):
                self.assertEqual(self.release(f"{kind}(scope): words (#4)"), Release("1.2.3", False))

    def test_docs_only_range_is_not_releasable(self):
        self.assertEqual(self.release("docs: a", "ci: b", "chore: c"), Release("1.2.3", False))

    def test_no_commits_is_not_releasable(self):
        self.assertEqual(self.release(), Release("1.2.3", False))

    def test_bang_before_the_colon_bumps_major(self):
        for message in ("feat!: drop the old port", "fix(net)!: reject old frames", "chore!: new toolchain"):
            with self.subTest(message=message):
                self.assertEqual(self.release(message), Release("2.0.0", True))

    def test_breaking_change_footer_bumps_major(self):
        for footer in ("BREAKING CHANGE: ports moved", "BREAKING-CHANGE: ports moved"):
            with self.subTest(footer=footer):
                self.assertEqual(
                    self.release(f"fix(net): move ports (#5)\n\nWhy.\n\n{footer}\nmore text\n"),
                    Release("2.0.0", True),
                )

    def test_breaking_change_footer_counts_on_any_subject(self):
        self.assertEqual(self.release("docs: note\n\nBREAKING CHANGE: x"), Release("2.0.0", True))
        self.assertEqual(self.release("Update things\n\nBREAKING CHANGE: x"), Release("2.0.0", True))

    def test_only_a_real_bang_or_footer_is_breaking(self):
        for message in (
            "fix: stop the crash!",
            "fix: mention BREAKING CHANGE: in prose",
            "fix: x\n\n BREAKING CHANGE: indented",
            "fix: x\n\nbreaking change: lowercase",
            "fix: x\n\nSee BREAKING CHANGE: later",
        ):
            with self.subTest(message=message):
                self.assertEqual(self.release(message), Release("1.2.4", True))

    def test_the_highest_bump_wins_and_resets_lower_numbers(self):
        self.assertEqual(self.release("fix: a", "feat: b", "docs: c"), Release("1.3.0", True))
        self.assertEqual(self.release("fix: a", "feat: b", "refactor!: c"), Release("2.0.0", True))
        self.assertEqual(self.release("feat: b", "fix: a", tag="v0.9.9"), Release("0.10.0", True))

    def test_non_conventional_subjects_release_nothing(self):
        for message in (
            "Update readme",
            "Merge pull request #3 from a/b",
            'Revert "feat: add a sampler"',
            "fix:no space",
            "fix : spaced",
            "feat(: broken scope",
            "",
        ):
            with self.subTest(message=message):
                self.assertEqual(self.release(message), Release("1.2.3", False))

    def test_type_case_and_line_endings_do_not_matter(self):
        self.assertEqual(self.release("Feat(Net): x\r\n\r\nbody\r\n"), Release("1.3.0", True))
        self.assertEqual(self.release("fix: x\r\n\r\nBREAKING CHANGE: y\r\n"), Release("2.0.0", True))

    def test_bad_tag_is_an_error(self):
        with self.assertRaises(ValueError):
            self.release("fix: a", tag="v1.2")


class NewestTag(unittest.TestCase):
    def test_orders_by_semver_not_text_or_date(self):
        self.assertEqual(newest_tag(["v0.9.0", "v0.10.0", "v0.2.0"]), "v0.10.0")
        self.assertEqual(newest_tag(["v0.10.0", "v0.9.0"]), "v0.10.0")
        self.assertEqual(newest_tag(["v1.9.10", "v1.10.0", "v1.9.9"]), "v1.10.0")

    def test_ignores_tags_that_are_not_plain_versions(self):
        tags = ["v2", "v2.0", "v3.0.0-rc1", "v01.0.0", "vfoo", "x9.9.9", "9.9.9", "v1.0.0+b", "v1.0.0"]
        self.assertEqual(newest_tag(tags), "v1.0.0")

    def test_none_without_a_version_tag(self):
        self.assertIsNone(newest_tag([]))
        self.assertIsNone(newest_tag(["v2", "latest"]))

    def test_a_tag_that_sorts_wrong_still_picks_the_right_bump(self):
        tag = newest_tag(["v0.9.0", "v0.10.0"])
        self.assertEqual(next_release(tag, ["fix: a"], "0.1.0"), Release("0.10.1", True))


GIT_ENV = {
    **os.environ,
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
    "GIT_AUTHOR_NAME": "Test",
    "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "Test",
    "GIT_COMMITTER_EMAIL": "test@example.invalid",
}


class CommandLine(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.repository = Path(directory.name) / "repository"
        self.repository.mkdir()
        self.git("init", "--quiet")
        (self.repository / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
        self.git("add", "Cargo.toml")

    def git(self, *arguments, cwd=None):
        subprocess.run(
            ["git", "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", *arguments],
            cwd=cwd or self.repository, env=GIT_ENV, check=True, capture_output=True, text=True,
        )

    def commit(self, subject, *paragraphs):
        arguments = ["commit", "--quiet", "--allow-empty", "-m", subject]
        for paragraph in paragraphs:
            arguments.extend(["-m", paragraph])
        self.git(*arguments)

    def run_script(self, *arguments, repository=None):
        return subprocess.run(
            [sys.executable, "-I", str(SCRIPT), "--repository", str(repository or self.repository), *arguments],
            env=GIT_ENV, capture_output=True, text=True,
        )

    def test_first_release_prints_the_workspace_version(self):
        self.commit("docs: start")
        result = self.run_script()
        self.assertEqual((result.returncode, result.stdout), (0, "version=0.1.0\nreleasable=true\n"), result.stderr)

    def test_reads_tags_and_commits_since_the_newest_tag(self):
        self.commit("feat: first")
        self.git("tag", "v0.9.0")
        self.commit("feat: second")
        self.git("tag", "v0.10.0")
        self.commit("fix(net): third (#1)", "Body text.")
        result = self.run_script()
        self.assertEqual((result.returncode, result.stdout), (0, "version=0.10.1\nreleasable=true\n"), result.stderr)

    def test_reads_a_breaking_footer_from_the_log(self):
        self.commit("feat: first")
        self.git("tag", "v0.10.0")
        self.commit("feat: second", "Body.", "BREAKING CHANGE: it moved")
        result = self.run_script()
        self.assertEqual(result.stdout, "version=1.0.0\nreleasable=true\n", result.stderr)

    def test_a_docs_only_range_keeps_the_tag_version(self):
        self.commit("feat: first")
        self.git("tag", "v0.10.0")
        self.commit("docs: later")
        result = self.run_script()
        self.assertEqual(result.stdout, "version=0.10.0\nreleasable=false\n", result.stderr)

    def test_no_new_commits_since_the_tag_is_not_releasable(self):
        self.commit("feat: first")
        self.git("tag", "v0.10.0")
        result = self.run_script()
        self.assertEqual(result.stdout, "version=0.10.0\nreleasable=false\n", result.stderr)

    def test_github_output_is_appended_and_nothing_is_printed(self):
        self.commit("feat: first")
        self.git("tag", "v0.1.0")
        self.commit("fix: second")
        output = Path(self.repository.parent) / "github-output"
        output.write_text("earlier=1\n")
        result = self.run_script("--github-output", str(output))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(output.read_text(), "earlier=1\nversion=0.1.1\nreleasable=true\n")

    def test_refuses_a_shallow_clone(self):
        self.commit("feat: first")
        self.commit("fix: second")
        shallow = self.repository.parent / "shallow"
        self.git("clone", "--quiet", "--depth", "1", self.repository.as_uri(), str(shallow), cwd=self.repository.parent)
        result = self.run_script(repository=shallow)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("full history", result.stderr)


if __name__ == "__main__":
    unittest.main()

# Security Policy

## Supported versions

`clap-layers` is pre-1.0. Only the latest published version receives fixes;
there are no long-term support branches. Before `1.0`, security fixes may ship
in a version that also contains breaking changes, because a `0.x` minor bump is
the only way to make one.

| Version | Supported |
| ------- | --------- |
| latest `0.x` | yes |
| anything older | no |

## Reporting a vulnerability

Please report privately, not in a public issue.

Use GitHub's [private vulnerability reporting][gh-report] on this repository,
which opens a channel visible only to the maintainers. If that is unavailable to
you, email **jchultarsky@gmail.com** with `clap-layers` in the subject.

[gh-report]: https://github.com/jchultarsky/clap-layers/security/advisories/new

Please include the crate version, the smallest configuration that reproduces the
problem, and what an attacker gains. A proof of concept helps but is not
required.

This is a single-maintainer project, not a company with an on-call rota. Expect
an acknowledgement within a week. If a report is valid I will agree a disclosure
timeline with you rather than sit on it, and credit you in the advisory and the
changelog unless you would rather I did not.

## What is in scope

The crate reads configuration from a file, the environment and the command line,
and hands the values to your program. Reports worth making include:

- A value reaching a field that a `no_env`, `no_file` or `no_cli` marker should
  have excluded. These markers are how a credential is kept out of a layer, so
  a leak between layers is a security bug, not a correctness nit.
- A panic reachable from untrusted configuration input. This crate returns
  `Result`; a config file should never be able to abort the host program.
- Anything causing a secret to be logged, printed or included in an error
  message. Errors quote the offending *value*, so a case where that value should
  not have been shown is in scope.

## What is not

- A malicious config file that sets a legitimate value to something harmful.
  Deciding whether `port = 22` is acceptable is your program's job, not this
  crate's.
- Advisories in dependencies with no exploitable path through this crate. Those
  are worth an ordinary issue; `cargo deny check` runs weekly and will usually
  have found them already.
- The documented fact that a command-line argument is visible in `ps` output.
  That is a property of the operating system; see `examples/sensitive_data.rs`
  for keeping credentials off the command line.

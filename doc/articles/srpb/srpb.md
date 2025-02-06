
# Enforce a Locked & Reproducible Environment on Every Python Run

<!--
# Stop Running Python Blind: Ensure Package Alignment with Every Python Execution
# Stop Running Python Blind: Ensure a Reproducible Environment with Every Python Execution
# Ensure a Reproducible Environment for Every Python Run
# Make Every Python Execution Predictable and Reproducible -->

For compiled languages, reproducible builds are required to establish a chain of trust between source code and binaries. Is it possible to have this in Python? While an interpreted language like Python runs byte code instead of binaries, what would it take to only run Python if the dependencies conformed to an explicit definition? Python supports such intervention in initialization, and the `fetter` command-line tool can now configure a virtual environment to either warn or exit before executing code with misaligned dependencies.

For many, daily use of Python involves writing and executing code in a virtual environment. If collaborating with others, the direct dependencies of this code are documented in a `requirements.txt` or `pyproject.toml` file. If using `uv`, `poetry`, or related tools, a lock file, pinning all direct and transitive dependencies, might also be defined. The only way to ensure reproducible behavior in Python (as well as reducing the risk of installing malware) is to validate installed virtual environment dependencies against a lock file. If we can do it fast enough, we should do it every time we run Python. That is what the `fetter site-install` command does.





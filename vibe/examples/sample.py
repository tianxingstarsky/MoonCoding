"""Module docstring example."""
import os
import sys
from typing import List


def greet(name: str) -> None:
    print(f"hello, {name}")


@decorator
def decorated(x):
    return x + 1


class Foo:
    def __init__(self):
        self.x = 1

    def method(self):
        return self.x


def main(argv: List[str]) -> int:
    greet("world")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
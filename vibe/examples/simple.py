import os
import sys
from typing import List


def add(a: int, b: int) -> int:
    return a + b


def sub(a: int, b: int) -> int:
    return a - b


def main(argv: List[str]) -> int:
    print(add(1, 2))
    print(sub(3, 1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
import sys
import os

if len(sys.argv) != 2:
    print("need 1 arguments")
    sys.exit(1)

match sys.argv[1]:
    case "icy_term":
        print("Icy Term")
    case "icy_draw":
        print("Icy Draw")
    case "icy_view":
        print("Icy View")
    case _:
        print("UNKNOWN APP")

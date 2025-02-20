import sys

if len(sys.argv) != 3:
    print("need 1 arguments")
    sys.exit(1)

match sys.argv[1]:
    case "icy_term":
        print("Icy_Term_" + sys.argv[2] + ".AppImage")
    case "icy_draw":
        print("Icy_Draw_" + sys.argv[2] + ".AppImage")
    case "icy_view":
        print("Icy_View_" + sys.argv[2] + ".AppImage")
    case _:
        print("UNKNOWN APP")

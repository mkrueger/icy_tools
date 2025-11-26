# IcyTerm Scripting API

IcyTerm supports Lua scripting for automating terminal sessions, such as BBS logins and interactions.

## Running Scripts

Scripts can be run in two ways:

1. **Command line**: `icy_term --run script.lua`
2. **Shortcut**: `Alt+R` (or `Cmd+R` on macOS) to open the script file dialog

## API Reference

### Connection Functions

#### `connect(name_or_url)`

Connects to a BBS by name (from address book) or URL.

```lua
connect("My BBS")           -- Connect by address book name
connect("bbs.example.com")  -- Connect by hostname
connect("telnet://bbs.example.com:23")  -- Connect by full URL
```

**Parameters:**
- `name_or_url` (string): Address book entry name or connection URL

**Returns:** The connection string used

---

#### `disconnect()`

Disconnects from the current BBS.

```lua
disconnect()
```

---

#### `is_connected()`

Checks if the terminal is currently connected.

```lua
if is_connected() then
    println("Connected!")
end
```

**Returns:** `true` if connected, `false` otherwise

---

### Data Transfer Functions

#### `send(text)`

Sends text to the remote system.

```lua
send("Hello\n")          -- Send text with newline
send("myusername\r\n")   -- Send username with CR+LF
```

**Parameters:**
- `text` (string): The text to send (can include escape sequences like `\n`, `\r`)

---

#### `send_key(key_name)`

Sends a special key to the remote system. The key is mapped according to the current terminal emulation (ANSI, PETSCII, etc.).

```lua
send_key("enter")    -- Send Enter key
send_key("f1")       -- Send F1 function key
send_key("up")       -- Send arrow up
```

**Supported keys:**
- Navigation: `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`
- Editing: `enter`, `return`, `tab`, `backspace`, `delete`, `escape`
- Function keys: `f1` through `f12`

**Returns:** `true` if the key was recognized and sent, `false` otherwise

---

#### `send_credentials(mode)`

Sends the stored credentials from the current address book entry.

```lua
send_credentials()     -- Send username + password (with delay)
send_credentials(0)    -- Same as above
send_credentials(1)    -- Send username only
send_credentials(2)    -- Send password only
```

**Parameters:**
- `mode` (optional, integer):
  - `0` (default): Send username, wait 500ms, send password
  - `1`: Send username only
  - `2`: Send password only

---

### Screen Functions

#### `wait_for(pattern, timeout_ms)`

Waits until a pattern appears on the screen. Supports regular expressions.

```lua
-- Wait for login prompt (max 30 seconds, default)
wait_for("login:")

-- Wait with custom timeout (10 seconds)
wait_for("Password:", 10000)

-- Using regex pattern
local match = wait_for("Welcome .* to our BBS")
if match then
    println("Found: " .. match)
end
```

**Parameters:**
- `pattern` (string): Text or regex pattern to search for
- `timeout_ms` (optional, integer): Timeout in milliseconds (default: 30000)

**Returns:** The matched text if found, `nil` if timeout occurred

---

#### `on_screen(pattern)`

Checks if a pattern is currently visible on the screen. Does not wait.

```lua
if on_screen("Error") then
    println("An error occurred!")
    return
end

if on_screen("Welcome") then
    println("Login successful!")
end
```

**Parameters:**
- `pattern` (string): Text or regex pattern to search for

**Returns:** `true` if pattern is found, `false` otherwise

---

#### `get_screen()`

Returns the current screen content as a buffer object.

```lua
local screen = get_screen()
-- Use with screen methods
```

**Returns:** A `LuaScreen` object representing the current screen

---

### Screen Manipulation Functions

These functions operate directly on the terminal screen. They are available as global functions for convenience.

#### `gotoxy(x, y)`

Moves the cursor to the specified position.

```lua
gotoxy(0, 0)      -- Move to top-left corner
gotoxy(40, 12)    -- Move to center of 80x25 screen
```

**Parameters:**
- `x` (integer): Column (0-based)
- `y` (integer): Row (0-based)

---

#### `print(text)` / `println(text)`

Prints text at the current cursor position. Supports PCBoard color codes (see below).

```lua
print("Hello")           -- Print without newline
println("Hello World!")  -- Print with newline
println("@X0FWhite text on black background")
```

**Parameters:**
- `text` (string): Text to print (supports PCBoard color codes and escape sequences)

---

#### `get_width()` / `get_height()`

Returns the screen dimensions.

```lua
local w = get_width()   -- Usually 80
local h = get_height()  -- Usually 25
println("Screen size: " .. w .. "x" .. h)
```

**Returns:** Width or height in characters

---

#### `set_char(x, y, char)` / `get_char(x, y)`

Sets or gets a character at the specified position.

```lua
set_char(10, 5, "X")          -- Set character at position
local ch = get_char(10, 5)    -- Get character at position
```

**Parameters:**
- `x`, `y` (integer): Position (0-based)
- `char` (string): Single character to set

**Returns:** (for `get_char`) The character at the position

---

#### `clear_char(x, y)`

Clears a character at the specified position (sets to space with default colors).

```lua
clear_char(10, 5)
```

---

#### `pickup_char(x, y)`

Picks up the character and its attributes at the specified position, setting them as current.

```lua
pickup_char(10, 5)  -- Pick up character, color, etc.
print("X")          -- Print with picked-up attributes
```

---

#### `set_fg(x, y, color)` / `get_fg(x, y)`

Sets or gets the foreground color at a position.

```lua
set_fg(10, 5, 12)           -- Set foreground to bright red (color 12)
local fg = get_fg(10, 5)    -- Get foreground color index
```

**Parameters:**
- `x`, `y` (integer): Position (0-based)
- `color` (integer): Color index (0-15 for standard colors)

---

#### `set_bg(x, y, color)` / `get_bg(x, y)`

Sets or gets the background color at a position.

```lua
set_bg(10, 5, 1)            -- Set background to blue (color 1)
local bg = get_bg(10, 5)    -- Get background color index
```

---

#### `fg_rgb(r, g, b)` / `bg_rgb(r, g, b)`

Sets the current foreground or background color using RGB values.

```lua
fg_rgb(255, 128, 0)   -- Set foreground to orange
bg_rgb(0, 0, 128)     -- Set background to dark blue
println("Colored text!")
```

**Parameters:**
- `r`, `g`, `b` (integer): RGB values (0-255)

---

#### `set_palette_color(index, r, g, b)` / `get_palette_color(index)`

Modifies or retrieves a palette color.

```lua
set_palette_color(1, 0, 128, 255)    -- Change color 1 to cyan
local r, g, b = get_palette_color(1) -- Get color 1's RGB values
```

**Parameters:**
- `index` (integer): Palette index (0-15 for standard, 0-255 for extended)
- `r`, `g`, `b` (integer): RGB values (0-255)

**Returns:** (for `get_palette_color`) Three values: r, g, b

---

#### `clear_screen()`

Clears the entire screen.

```lua
clear_screen()
```

---

### Layer Functions

These functions manage screen layers for advanced effects.

#### `layer_count()`

Returns the number of layers.

```lua
local count = layer_count()
```

---

#### `set_layer(index)` / `get_layer()`

Sets or gets the current active layer.

```lua
set_layer(1)              -- Switch to layer 1
local current = get_layer() -- Get current layer index
```

---

#### `set_layer_position(layer, x, y)` / `get_layer_position(layer)`

Sets or gets the position offset of a layer.

```lua
set_layer_position(1, 10, 5)        -- Move layer 1 offset
local x, y = get_layer_position(1)  -- Get layer 1 position
```

---

#### `set_layer_visible(layer, visible)` / `get_layer_visible(layer)`

Sets or gets the visibility of a layer.

```lua
set_layer_visible(1, false)         -- Hide layer 1
local vis = get_layer_visible(1)    -- Check if visible
```

---

### PCBoard Color Codes

The `print()` and `println()` functions support PCBoard color codes for colorized output:

#### Format: `@Xbf`

- `@X` - Color code prefix
- `b` - Background color (hex digit 0-F)
- `f` - Foreground color (hex digit 0-F)

#### Color Values

| Code | Color         | Code | Color           |
|------|---------------|------|-----------------|
| 0    | Black         | 8    | Dark Gray       |
| 1    | Blue          | 9    | Light Blue      |
| 2    | Green         | A    | Light Green     |
| 3    | Cyan          | B    | Light Cyan      |
| 4    | Red           | C    | Light Red       |
| 5    | Magenta       | D    | Light Magenta   |
| 6    | Brown         | E    | Yellow          |
| 7    | Light Gray    | F    | White           |

#### Examples

```lua
println("@X0FWhite on Black")
println("@X1FWhite on Blue")
println("@X4EYellow on Red")
println("@X0CLight Red@X0F then White")
println("@X02Green @X0EYellow @X09Light Blue")
```

---

### Escape Sequences

Standard escape sequences are supported in all string functions:

| Sequence | Description     |
|----------|-----------------|
| `\n`     | Newline (LF)    |
| `\r`     | Carriage Return |
| `\t`     | Tab             |
| `\\`     | Backslash       |
| `\"`     | Double quote    |

#### Examples

```lua
send("username\r\n")     -- Send with CR+LF
println("Line 1\nLine 2") -- Two lines
print("Column1\tColumn2") -- Tab-separated
```

---

### Utility Functions

#### `sleep(ms)`

Pauses script execution for the specified duration.

```lua
sleep(1000)    -- Wait 1 second
sleep(500)     -- Wait 500 milliseconds
```

**Parameters:**
- `ms` (integer): Duration in milliseconds

---

## Example Scripts

### Auto-Login Script

```lua
-- Connect to BBS
connect("My Favorite BBS")

-- Wait for connection and login prompt
if wait_for("login:", 15000) then
    send_credentials(1)      -- Send username
    
    if wait_for("Password:", 5000) then
        send_credentials(2)  -- Send password
        
        -- Check for successful login
        sleep(2000)
        if on_screen("Welcome") then
            println("Login successful!")
        elseif on_screen("Invalid") or on_screen("Error") then
            println("Login failed!")
        end
    end
else
    println("Connection timeout")
end
```

### Menu Navigation Script

```lua
-- Wait for main menu
wait_for("Main Menu")

-- Navigate to message area
send("M")  -- Messages
sleep(500)

if wait_for("Message Areas") then
    send("1")  -- Select first area
    send_key("enter")
end
```

### Error Handling Script

```lua
connect("Some BBS")

-- Wait for either login prompt or error
local result = wait_for("(login:|Connection refused|busy)", 10000)

if result == nil then
    println("Timeout - no response")
elseif on_screen("refused") or on_screen("busy") then
    println("Could not connect: " .. result)
else
    -- Proceed with login
    send_credentials()
end
```

## Tips

1. **Use `sleep()` between actions** - Many BBS systems need time to process input
2. **Use `on_screen()` for error handling** - Check for error messages after critical operations
3. **Use regex patterns** - Both `wait_for()` and `on_screen()` support regex
4. **Store credentials in address book** - Use `send_credentials()` instead of hardcoding passwords
5. **Handle timeouts** - `wait_for()` returns `nil` on timeout, check for this

## Terminal Emulation

The `send_key()` function automatically maps keys to the correct escape sequences based on the current terminal emulation:

- **ANSI/VT100**: Standard ANSI escape sequences
- **PETSCII**: Commodore 64/128 key codes
- **ATASCII**: Atari 8-bit key codes
- **ViewData**: Viewdata/Prestel codes
- **Mode7**: BBC Micro Mode 7

The emulation is determined by the connection settings in the address book.

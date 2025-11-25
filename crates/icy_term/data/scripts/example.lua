-- Example icy_term Lua script
-- This script demonstrates the available scripting functions
--
-- Available functions:
--   send(text)                  - Send text to terminal (\r for Enter, \x1b for Escape)
--   sleep(ms)                   - Sleep for specified milliseconds
--   wait_for(pattern, timeout)  - Wait for regex pattern, returns match or nil on timeout
--   get_screen()                - Get current screen buffer (width, height, get_char, get_line, get_text)
--   is_connected()              - Check if terminal is connected
--   disconnect()                - Disconnect from current BBS
--   connect(name_or_url)        - Connect to BBS by address book name or URL
--   send_credentials(mode)      - Send credentials from address book:
--                                   0 = username + password (default)
--                                   1 = username only
--                                   2 = password only
--   clear_buffer()              - Clear the incoming data buffer
--   log(message)                - Log message to script output
--   print(message)              - Print message to script output

print("Hello from icy_term scripting!")
log("Script started")

-- Check connection status
if is_connected() then
    log("Terminal is connected")
    
    -- Get current screen content
    local screen = get_screen()
    log("Screen size: " .. screen.width .. "x" .. screen.height)
    
    -- Example: Read a character from screen
    local ch = screen:get_char(0, 0)
    log("Character at (0,0): " .. ch)
    
    -- Example: Send some text
    -- send("Hello\r\n")
    
    -- Example: Wait for a pattern with 5 second timeout
    -- local result = wait_for("login:", 5000)
    -- if result then
    --     log("Found login prompt!")
    --     send("username\n")
    -- else
    --     log("Timeout waiting for login prompt")
    -- end
else
    log("Terminal is not connected")
    
    -- Example: Connect to a BBS by name (from address book)
    -- connect("My Favorite BBS")
    
    -- Example: Connect to a BBS by URL
    -- connect("telnet://bbs.example.com:23")
    -- connect("ssh://user@bbs.example.com")
    
    -- If no protocol is specified, telnet is assumed:
    -- connect("bbs.example.com:23")
    
    -- After connecting, you can send stored credentials:
    -- send_credentials()  -- sends username + password
end

-- Sleep for 1 second
sleep(1000)

log("Script finished")
print("Done!")

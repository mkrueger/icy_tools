-- Login script for 20forbeers BBS
-- This script demonstrates automatic BBS login
--
-- Requirements:
-- - "20forbeers" must be in your address book with username and password configured
--
-- Usage: Run this script from the menu (Scripts > Run Script) or press Cmd+R
--        It'll bring you to the main window.
log("Starting 20forbeers login script...")

-- Connect to the BBS (looks up "20forbeers" in address book)
local url = connect("20forbeers")
log("Connecting to: " .. url)

-- Wait for the initial "Press" prompt and send 2x Escape
log("Waiting for initial screen...")
local result = wait_for("Welcome", 15000)
if not result then
    log("ERROR: Timeout waiting for welcome screen")
    return
end
send("\x1b")

local result = wait_for("Press", 15000)
if not result then
    log("ERROR: Timeout waiting for initial screen")
    return
end
log("Found initial screen, sending escape keys...")
send("\x1b\x1b")  -- 2x Escape

-- Wait for CP437 selection and confirm with Enter
log("Waiting for character set selection...")
result = wait_for("CP437", 5000)
if not result then
    log("ERROR: Timeout waiting for CP437 prompt")
    return
end
log("Found CP437 prompt, confirming...")
send("\r")
 
-- cancel scroller
sleep(100)
send(" ") 

-- Wait for username prompt and send credentials
log("Waiting for username prompt...")
result = wait_for("USeRNaMe", 15000)
if not result then
    log("ERROR: Timeout waiting for username prompt")
    return
end
log("Found username prompt, sending credentials...")

-- Send username and password from address book
-- send_login() sends both username and password (with delay between)
-- send_username() sends username only
-- send_password() sends password only
send_login()

-- invisible login 
sleep(100)
send_key("left")
send_key("enter")

-- rocket login
sleep(100)
send_key("left")
send_key("enter")

result = wait_for("rEAD eM?", 15000)
if not result then
    log("ERROR: Timeout waiting for username prompt")
    return
end
send_key("enter")

sleep(100)
send_key("enter")

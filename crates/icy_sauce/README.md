# icy_sauce

Library for handling SAUCE – Standard Architecture for Universal Comment Extensions

It's an old format to store meta data information in certain file types. I need that now in several projects targeted at the BBS scene.
So I splitted that out.

This library depends on the awesome bstr crate the strings are usually CP437 encoded that needs to be handled by the user.

## TODO

* Add other capabilities for Audio/Archive/Executables etc. - I don't need those not sure if there are even files that use them.
* Code review - I'm always interested in feedback. I wrote that years ago as part of one of my first rust programs.

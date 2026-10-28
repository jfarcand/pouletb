-- SPDX-License-Identifier: Apache-2.0
--
-- Pouletb.app: opens .torrent files and magnet links in a new Terminal window
-- running `pouletb`, so the download and its progress bar are visible.
-- Built and installed by install.sh.

-- Terminal runs the user's login shell, which normally has ~/.cargo/bin on
-- PATH already; the prefix covers shells that do not, plus Homebrew.
property launcher : "PATH=\"$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\" pouletb "

on run
	display dialog "Pouletb downloads .torrent files and magnet links with the pouletb command, in a Terminal window." & return & return & "Double-click a .torrent file or click a magnet link to start." buttons {"OK"} default button 1 with title "Pouletb"
end run

on open theFiles
	repeat with f in theFiles
		startDownload(quoted form of POSIX path of f)
	end repeat
end open

on open location theURL
	startDownload(quoted form of theURL)
end open location

on startDownload(arg)
	set command to launcher & arg
	set wasRunning to application "Terminal" is running
	tell application "Terminal"
		if wasRunning then
			do script command
		else
			-- A cold start opens its own empty window: reuse it rather than
			-- leaving a stray second one next to the download.
			activate
			repeat 50 times
				if (count of windows) > 0 then exit repeat
				delay 0.1
			end repeat
			if (count of windows) > 0 then
				do script command in front window
			else
				do script command
			end if
		end if
		activate
	end tell
end startDownload

# Keys and mouse

Every key and click per screen, from the original game's code. "Port" = the port does it (no = not yet).


## Tactical

| Key / click | What it does | Port |
|---|---|---|
| F1 to F8 (or Alt+F1 to Alt+F8) | Choose merc 1 to 8. The merc must be alive and on the map. | yes |
| Left click on a merc face (portrait columns) | Chooses that merc, the same as its key F1 to F8. | yes |
| Left click on a tile | Walk cursor. The first click marks the tile and shows the path and AP cost. With Safety Move on, a second click on the same tile starts the walk. | yes |
| Left click on another merc | Chooses that merc. If the click fails, the status line shows "Not available!". | yes |
| Alt + left click on the selected merc | Changes the posture of the selected merc at once, with no confirm click. | yes |
| Left click with the door or hand cursor | The merc uses the door or object on the tile. For some objects the hand cursor needs a second click on the same tile. | yes |
| Right click on a tile | Switches the cursor between the walk cursor and the weapon cursor, if the merc holds an item in the main hand. | yes |
| Shift + left click on a tile | The merc sneaks to the tile. The step AP cost can differ from a normal walk. | yes |
| Right click on the selected merc | Asks to change the posture. Left click on the same tile to confirm, Escape to cancel. | yes |
| Hold right button + left click | The selected merc turns toward the mouse tile. A fight turn costs 1 AP. | yes |
| Hold right button, move the mouse down, release | Look mode. The game shows the overview of the whole sector map. | yes |
| Right click on a sign, or with the hand cursor | Zooms the ground item or shows the sign text. Any key or click ends it. | yes |
| Right click on a merc portrait | Opens the switch view of that merc: max aim, rsv pts (stop at low AP) and mute. Left click a switch to flip it. Right click closes the view. | yes |
| Hold left button on a merc bar area | Shows the stat numbers of that merc while you hold the button. | yes |
| Left drag from the map toward an edge | Leaves the sector over that edge if the squad can. An arrow shows while you hold the button. A "?" cursor means you cannot leave there. | yes |
| Hold left button + right click | ALL-move. The whole squad walks to the mouse tile. Not available in a fight. | yes |
| Rest the mouse on a tile | The status line shows the AP cost and, with Show Paths on, path marks show. | yes |
| Click on a button of the top strip (DONE, TRAV, INV, MAP, OPT, ABORT) | Does the same as the key D, T, I, M, F10 or A. DONE works only in a fight on your turn. | yes |
| Click on the sector box of the top strip | Pauses the game, like the key P. | yes |
| Left click on a bottom strip box | Takes the item from the box, or puts the cursor item into it. Ctrl takes one item from a stack. | yes |
| Right click on a bottom strip box | First box: the gun goes to a free box. Last box: swaps the two hands. Other boxes: opens the inventory screen. | yes |
| Escape | Stops the selected merc on the next tile. Also cancels an open aim or walk confirm. It never quits the game. | yes |
| Arrow keys | Move the map view one tile. A second press within 70 ms waits; with the Fast Scroll switch there is no wait. | yes |
| Mouse at a screen edge | Move the map view one tile in that direction after 200 ms, then one tile every 70 ms while the mouse stays at the edge. | yes |
| C | Time compression on or off, outside a fight. In a fight the status line shows "Can't compress time, there are enemies nearby!". | yes |
| Any key or mouse click during time compression | Turns time compression off. The key or click does nothing else. | yes |
| I | Opens the inventory screen of the selected merc. The merc needs at least 15 hit points. | yes |
| M | Opens the guard screen (island map with the guard panel). | yes |
| T | Traverse popup. Pick NORTH, WEST, EAST or SOUTH (keys N, W, E, S; Escape cancels). The squad must stand together at that map edge. | yes |
| A | Asks "ABORT this day?". Yes asks "RESTART this day?". Yes restarts the day, No goes back to the office. | yes |
| S | Shows the stat numbers of all mercs at once. Any key or click ends it. | yes |
| L | Shows the text of the last quote again. No voice plays. | yes |
| Space | Selects the next merc (wraps from the last to the first). In a fight it shows the path of its open walk order. Not with an item on the cursor. | yes |
| Enter | In a fight: the view goes to the next enemy in sight (the next one each press). With none in sight it shows "No enemies visible!". | yes |
| X | The selected merc swaps places with the merc or guard under the mouse, next to it. Both pay the step AP. Too far, too weak or too few AP shows a message. | yes |
| + and - | Music volume up or down by 1 (0 to 100, start 100). The first press sets the volume, so the music can get quieter even with +. | yes |
| . and , | Sound volume up or down by 1 (0 to 100, start 70). The first press sets the volume, so the sounds get quieter even with "." . | yes |
| V | Shows the version text "Release  v1.13b  Sep 07-95" in the message box. | yes |
| P | Pause. A wristwatch with the time shows at the map corner. Any key or click ends the pause. | yes |
| Z | Zooms the item or sign on the tile under the mouse. Any key or click ends it. | yes |
| F9 | Shows or hides the line "how many enemies each merc sees". | yes |
| F10 | Opens the Control Panel with the 16 switches. | yes |
| PgUp | Next view mode: normal, wide map, full-screen map, overview. | yes |
| PgDn | Previous view mode. | yes |
| Insert | Overview of the whole sector map. | yes |
| Tab | Switches between the normal screen and the last wide view. In the overview it switches the merc filter. | yes |
| Left click in the overview | Walks the selected merc there, or selects a merc. Right click jumps to that place in the normal view. | yes |
| Alt+S | Quick Save, then Y to confirm. Not in a fight. | yes |
| Alt+R | Quick Restore, then Y to confirm. Needs a quick save file. | yes |
| Alt+X | Asks "Exit to DOS?". Yes asks "Save day in progress?" (yes saves the quick save), then the game ends with a farewell line. | yes |

## Tactical (fight)

| Key / click | What it does | Port |
|---|---|---|
| D | Ends your turn. | yes |
| Left click on an enemy with the weapon cursor | Starts the aim. The AP number shows. Left click again fires. With Fast Firing on, the first click fires. | yes |
| Right click while aiming | Adds one more aim step (up to 4 extra AP). After the maximum it goes back to the lowest aim. | yes |
| Escape while aiming | Cancels the shot. | yes |
| Hold left button + right click | Shows "ALL-MOVE not available during Turn-based". The squad does not move. | yes |
| C | Shows "Can't compress time, there are enemies nearby!". | yes |

## Inventory

| Key / click | What it does | Port |
|---|---|---|
| Left click on an item | Picks the item up, or puts the cursor item down in that place. Shift swaps the cursor item and the place item. | yes |
| Ctrl + left click on a stack | Takes one item from a stack of two or more. | yes |
| Right click on an item | Shows the item info box and plays the item name. | yes |
| Left click on the mouth | The merc drinks the item on the cursor. Costs 5 AP. | yes |
| Left click on the body | The merc uses a camouflage kit. Costs 10 AP. | yes |
| Left click on a merc slot at the screen side | Shows that merc. | yes |
| T | Turns the Subtitles option on or off and shows "Text ". | yes |
| Escape | Closes the inventory screen. With an item on the cursor it shows "Better put that down first!". | yes |

## Control Panel

| Key / click | What it does | Port |
|---|---|---|
| Left click on a switch | Turns the switch ON or OFF. Sound FX and Subtitles cannot both be off. | yes |
| Left click on Continue Game | Closes the panel and goes back to the game. | yes |
| Left click on Quick Save or Quick Restore | Closes the panel and does the quick save or restore. Quick Save shows only when allowed; Quick Restore only with a quick save file. | yes |
| Escape | Closes the panel. | yes |

## Guard screen

| Key / click | What it does | Port |
|---|---|---|
| Right click on a sector you own | Takes one guard into the hand. "Awaiting dest" counts the guards in the hand. | yes |
| Left click on a linked sector | Sends the guard in the hand there. The travel hours show while the mouse rests on a sector. | yes |
| Left click on the sector the guard came from | Puts the guard back. | yes |
| Ctrl + right click on a sector | Calls the last guard sent to that sector back home. | yes |
| Enter | Leaves the screen and the guards set off. With guards still in the hand it shows "There are Guards without destinations!". | yes |

## Island map

| Key / click | What it does | Port |
|---|---|---|
| Left click on a sector | Depends on the tab. Team tab: sets the starting sector for the team. Tappers tab: adds 3 tappers. Guards tab: adds a guard. | yes |
| Right click on a sector | Depends on the tab. Team tab: clears the starting sector. Tappers tab: removes 3 tappers. Guards tab: removes a guard. | yes |
| T | Subtitles switch on or off, with the message "Text on" or "Text off". The guard screen does the same. | yes |
| Enter | Starts the day after the checks. A box asks for the answer (Y, O or Enter = yes; N or Escape = no). | yes |
| Click on the start (OK) button (bottom right) | Starts the day, like Enter. | yes |
| Click on the field box at the bottom left | Opens the assignment screen. | yes |
| L | Shows the text of the last quote again. | yes |
| Alt+X | Asks "Exit to DOS?". Yes ends the game with a farewell line. With no quick save, the saves of the current day are deleted. | yes |

## Assignment screen

| Key / click | What it does | Port |
|---|---|---|
| Click on a merc face | Shows the items of that merc. | yes |
| Left click on an item, then on a free place | The item sticks to the mouse, then goes into the place. | yes |
| Right click on an item | Shows the item info box. Any click closes it. | yes |
| Click on the duty name above a face | Opens the duty menu. FIRE asks "Fire <name>?" with Y or N. | yes |
| E | Puts all items of the selected merc into the item store. | yes |
| L | Shows the text of the last quote again. | yes |
| Enter or the exit button | Leaves the screen. With nobody on duty a question asks to confirm. | yes |
| Alt+X | Asks "Exit to DOS?". Yes ends the game with a farewell line. With no quick save, the saves of the current day are deleted. | yes |

## Office

| Key / click | What it does | Port |
|---|---|---|
| Click on the bed | Sleep. With no team the box says "You try to sleep, but ...". | yes |
| Click on the top half of the calendar | Save Game: click a slot row, type a name, Enter saves. Right click or Escape cancels. | yes |
| Click on the bottom half of the calendar | Restore Game: click a row to load that game. | yes |
| Click on the papers on the desk | View Team: the team ledger. Any click, Enter, Escape or Space closes it. | yes |
| Click on the laptop | Opens the A.I.M. screen. It works only when the A.I.M. link is on for the day. | yes |
| Click on the door | Leave: back to the main menu. | yes |
| Click on the box on the wall | Opens the Control Panel switch box. Click a lever to flip a switch. | yes |
| V | Shows the message box "Release  v1.13b  Sep 07-95" (the text of JA.EXE). | yes |
| Alt+X | Ends the game at once (no question, no save). | yes |
| L (on the View Team page) | Shows the text of the last quote again over the page. | ? |

## A.I.M.

| Key / click | What it does | Port |
|---|---|---|
| Left, Up, PgUp | Shows the previous merc. | yes |
| Right, Down, PgDn | Shows the next merc. | yes |
| Right click on the face | Shows the next merc. Hold the button to repeat. | yes |
| Click on the face | Starts the interview and the hire. | yes |
| Click on INFO or GEAR (BIO) | INFO opens the statistics page. GEAR and BIO switch the text panel between gear and biography. | yes |
| Escape or click on EXIT | Leaves the screen. Pending hires are paid with the helicopter charge. | yes |
| V | Shows the version message. | yes |
| L | Shows the text of the last quote again. | no |
| T | Turns the Subtitles option on or off and shows "Text  on" or "Text  off". | yes |
| Alt+X | Asks "Exit to DOS?". Yes ends the game with a farewell line. With no quick save, the saves of the current day are deleted. | yes |

## Payroll

| Key / click | What it does | Port |
|---|---|---|
| Click on a face (Not enough funds) | Asks "Don't pay <name>?". Y sends the merc away unpaid. | yes |
| Click on a face (after OK on the day replay) | Asks "Fire <name>?". Y fires the merc; the team closes up. The mouse on a face shows the "fire" tooltip. | yes |

## Day end

| Key / click | What it does | Port |
|---|---|---|
| L, T, Alt+X (day replay and payroll screens) | L repeats the last quote, T switches the subtitles, Alt+X asks "Exit to DOS?". | yes |

## Main menu

| Key / click | What it does | Port |
|---|---|---|
| Up, Down | Moves the lit row. The mouse jumps to the row. | yes |
| Enter | Picks the lit row: Start New Game, Continue Game, or Exit. | yes |
| Left click on a row | Picks that row. | yes |
| Up, Down, Enter on the difficulty and quick-save screens | Moves the lit row and picks it. The choice cannot change later. | yes |
| Y or N at "Continue last day in progress?" | Y continues from the quick save. N asks again; Y deletes the quick save and day files. | yes |

## Message / question box

| Key / click | What it does | Port |
|---|---|---|
| Y, O or Enter | Answers yes. | yes |
| N or Escape | Answers no. | yes |
| Any key or click (message box) | Closes the message box. | yes |

## Day replay

| Key / click | What it does | Port |
|---|---|---|
| Escape | Stops the replay animation and skips the rest of the replay. | yes |

## Any screen

| Key / click | What it does | Port |
|---|---|---|
| Escape (film or credits) | Stops the film, the intro or the credits. | yes |

## Debug (level 4)

| Key / click | What it does | Port |
|---|---|---|
| Ctrl+B, Ctrl+A, Ctrl+L, Ctrl+D (in this order) | Turns debug level 4 on. A wrong key in the chain sets the level to 0. Ctrl+D at level 4 turns it off. Nothing shows. | yes |
| Ctrl+R | Gives every merc full clips. | yes |
| Alt+L | Heals the selected merc fully. | yes |
| K | Kills the soldier on the tile under the mouse. | yes |
| @ (Shift+2) | Breaks the object (door) on the tile under the mouse. | yes |
| Ctrl+T | Teleports the selected merc to the tile under the mouse. | yes |
| Ctrl+F1 | Turns Alt+S quick save in a fight on or off. | yes |
| Alt+Q | Shows or hides roofs. | yes |
| Alt+E | Shows or hides all enemies. | yes |
| E, F, G | Show tile numbers (overlay, struc, gridno) of the tile under the mouse in message boxes. | yes |
| Ctrl+P | Centers the view on the selected merc and shows four pages of soldier data. | yes |
| Ctrl+O | Shows the opponent lists of the soldier under the mouse in message boxes. | yes |
| Shift + left click on a soldier | Opens the inventory screen of that soldier (also an enemy), then the four data pages. | yes |

## Payroll

| Key / click | What it does | Port |
|---|---|---|
| Escape during the stat count | Counts the rest of the stats without the pauses and the clicks. | yes |

## Control Panel switches (F10)

| Switch | What it does |
|---|---|
| Sound FX | ON: sound effects play (shots, steps, doors). OFF: no sound effect starts, but speech and music still play. |
| Music | ON: the music plays, and turning it ON starts the theme at once. OFF: the music stops. |
| Fast Scroll | ON: the map view scrolls on every arrow key at once. OFF: after one scroll step the next waits 70 ms. |
| Alt Scroll | ON: a waiting scroll step runs only while Alt is held. OFF: it runs when Alt is up and the 200 ms hover time is over. The mouse-edge scroll uses the same rule. |
| Shadows | ON: the map draws shadows under soldiers and objects. OFF: no shadows. |
| Subtitles | ON: speech shows as text, and step noises show as words such as "clunk". OFF: a face slides in for speech and the texts stay away. |
| Item Info | ON: a right click on an item shows the item info box. OFF: you only hear the item name, except for picture items such as letters. |
| Show Paths | ON: the walk cursor marks the path tiles on the map. OFF: no path marks show. |
| Pal Cycling | ON: water colours cycle and random ambient sounds play. OFF: the colours stay still and no ambient sound plays. |
| Fast Anims | ON: walking animations run at half the frame delay, so mercs move faster on screen. OFF: normal delay. |
| Follow move | ON: in a fight the view follows your selected merc while it moves. OFF: the view follows only the enemies you see. |
| Step Sounds | ON: you hear footstep sounds of soldiers you can see. OFF: no footstep sounds. |
| Day Replays | ON: after sunset the game replays the day on the island map. OFF: no replay. |
| Fast Firing | ON: a left click with the weapon cursor fires at once, also in a fight. OFF: in a fight the first click starts the aim. |
| Safety Move | ON: a walk needs a second left click on the same tile. OFF: one click starts the walk. |
| Merc OKs | ON: a merc answers with a spoken "OK" when you give a move order with a marker. OFF: no answer. |
| Allow save (ALLOWSAVE) | Program option. ON: a loaded game from before v1.14 with no quick-save mode set gets quick save in a fight. The original prints a notice and waits for a key; the port prints it to stderr. |

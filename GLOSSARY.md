# flipsaver

A Windows screensaver that shows the time as split-flap displays: a two-box clock or a world-clock
board.

## Language

**Flap**:
One split-flap position showing a single value: a clock box's hour or minute, or one board
character. It holds the value it shows and, while it changes, the flip from the old value.
_Avoid_: Cell, box (for the animated unit), CellState

**Flap row**:
The ordered flaps of one face: two for the clock (hours, minutes), one per character for the board.
_Avoid_: Grid (that is the board's on-screen geometry)

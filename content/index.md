---
page: Home
---

Oathbreaker is a traditional roguelike with a focus on stealth, heavily inspired
by games like Harmonist. As a prisoner in a goblin outpost, you must escape your
impending execution --- preferably creating as small a disturbance possible in
the process.

### What?

Oathbreaker is a

- **traditional roguelike**: This means a game that's **turn-based**, takes
  place on a **grid**, is **brutally difficult**, yet uses **procedural
  generation** to take the edge off of **permadeath** and make each run fresh
  and enjoyable.

  Other examples:
  - [NetHack](https://en.wikipedia.org/wiki/NetHack),
  - [DCSS](https://crawl.develz.org/),
  - or [Cogmind](https://www.gridsagegames.com/cogmind/).

- **focused on stealth**: unlike other roguelikes where killing enemies is
  required to gain XP, in Oathbreaker one must simply move upwards and escape.
  Enemies are quite strong and the player's health is limited, so there is a strong
  incentive to avoid fights wherever possible. Progress comes through
  exploration and by finding equipment rather than working through hordes of
  enemies.

Some interesting mechanics are:

- **Item-based**. Your spells come from rings (of which you can have *six*).
  Your ability to channel magic comes from <span class='m-gold'>golden</span>
  items. Items with <span class=m-blue>`rElec`</span> or <span
  class=m-red>`rFire`</span> don't just give resistance, they also increase the
  power of relevant spells.
- **Highly dynamic gameplay**. Your fortunes can change on a dime. Not being
  spotted by an enemy at a crucial moment can be the difference between exiting
  the level with one HP or ten. Since your maximum HP only grows to 18 HP, this
  is significant!
- **Reactive environment**. Enemies react to your presence, becoming more
  careful or asking for reinforcements. Coroners examine bodies that you leave
  behind, and engineers build mechanical sentries to guard chokepoints.

### Where?

**NOTE**: Oathbreaker is in *public beta*. Bugs are to be expected, and many
features are incomplete. (For example, there isn't a way to save the game yet at
the moment.)

- [Download](https://github.com/kiedtl/roguelike/releases/tag/v4.1.0)

Only Windows and Linux are supported. macOS, terminal support, and maybe even
a web version are possible future projects.

### Gameplay

The game has 8-direction movement, so `qweasdzxc` keys (or Vim movement) will
do. Other common keys:

- `i` for inventory,
- `v` to view/examine a tile,
- `,` to pickup items,
- `SPACE` to see spells,
- `Esc` for a help screen.

A guide is on the main screen, which discusses many topics new topics find
confusing. (Don't worry, there's plenty *not* discussed that's left for you to
discover on your own!)

Also, there is basic mouse support! Most items in the HUD can be clicked on for
a simple help message.

**Goal**: find the stairs on each level, moving upwards as soon as you're able
to. Rings on each floor will grant you spells; find them if possible. There are
**8 required levels** (and 14 optional ones); once you reach `1/Prison`, find
the exit stairs and you've won! (almost!)

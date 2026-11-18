# Krople: notes for the real app

Krople is a small Windows 11 helper that runs next to DrEryk at an eye doctor's practice; the mockup is `Krople – timer gabinetu.html` in the repo root.

## Facts
- Two protocols:
  - **1 kropla:** one drop, then a 15-minute wait, then "Gotowy do badania", then "Zbadany".
  - **3 krople co 10 min:** a drop every 10 minutes. Each time a timer runs out the card shows "Kropić teraz", and the doctor confirms with "Zakropione". After the 3rd drop there's a 15-minute wait, then "Gotowy do badania".
- After a timer runs out, overdue time counts up ("+2 min").
- A card is red when either a drop is due or the patient is ready for the exam. The icon turns red when any card is red (when minimized: the user's rule. Also while open: Claude's call).
- Patient label: "Inicjały lub imię" (initials or first name). No surnames, nothing else identifying.
- Running timers are not saved. After a restart the list is empty.
- The settings page (gear icon) edits the 10-minute interval and the 15-minute wait. Saved settings only, never patients.
- Chosen card style: "Wariant: oko z postępem" (pie progress circle).
- Top bar: a minimize icon button next to the gear, with no "DrEryk" text. Quit goes on the settings page (Claude's call).
- Polish UI.

## Unconfirmed calls

The following are Claude's calls, awaiting the user's confirmation:

- The icon turns red while the main window is open too, not only when minimized.
- The number on the icon shows how many cards are red (ready count).
- Quit lives on the settings page.
- Timers run in Rust (the app's backend), not in JavaScript, because Windows throttles JavaScript in minimized windows.

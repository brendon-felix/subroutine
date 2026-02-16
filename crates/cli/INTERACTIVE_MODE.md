# Interactive Mode

Interactive mode provides a guided, user-friendly experience for working with Subroutine through conversational prompts using the `dialoguer` crate.

## Starting Interactive Mode

You can start interactive mode in two ways:

1. Run the CLI without any commands:
   ```bash
   subroutine-cli
   ```

2. Explicitly start interactive mode:
   ```bash
   subroutine-cli interactive
   ```

## Features

### 📝 Create a new action

Guides you through creating an action with executive function-aware attributes:

- Action type (task, activity, habit, event)
- Duration (using Fibonacci scale: 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144 minutes)
- Energy impact (-5 to +5: draining to energizing)
- Attention level required (1-5)
- Transition difficulty (1-5: how hard to start/stop)
- Enjoyment after starting (-5 to +5)
- Importance (1-5)
- Urgency growth (whether it becomes more urgent over time)

### ⏰ Create an instance (schedule an action)

Schedule an existing action:

- Select from your actions
- Choose the source (manual, routine, suggested)
- Optionally add to pipeline

### 📊 Capture current context

Record your current context including:

- Time of day (morning, afternoon, evening, night)
- Environment (quiet, noisy, social, solitary, indoors, outdoors, home, work)
- Location (home, work, transit, store, outdoors, other)
- Energy level (0.0-1.0)
- Attention capacity (0.0-1.0)
- Optional mental state link

### 🧠 Record mental state

Track your mental state:

- Select existing mental state or create a new one
- Record intensity (1-5)
- Automatically timestamped

### 🎯 View and work with pipeline

Manage your pipeline:

- View all pipeline items
- See details of specific items
- Mark items as completed (removes from pipeline)
- Add new instances to pipeline

### 📋 Quick action capture (batch mode)

Rapidly add multiple actions:

- Enter action titles one after another
- Skip detailed attributes for speed
- Optionally schedule actions after creation

### 🔍 Explore existing actions

Browse and manage your actions:

- View detailed information about any action
- Schedule actions from the list
- Delete actions (with confirmation)
- Navigate with keyboard

## User Experience

Interactive mode uses:

- **ColorfulTheme** for visually appealing prompts
- **Multi-select** for choosing multiple options (environments, locations, times)
- **Select menus** for single choices with arrow key navigation
- **Input prompts** for text entry
- **Confirmation dialogs** for destructive actions
- **Clear visual feedback** with emojis and formatted output

## Navigation

- Use **arrow keys** to navigate menus
- Press **Space** to select items in multi-select menus
- Press **Enter** to confirm selections
- Press **Ctrl+C** to exit at any time

## Design Philosophy

Interactive mode embodies Subroutine's core philosophies:

1. **Reduce decision fatigue** - Guided prompts with sensible defaults
2. **Executive function awareness** - Questions designed around ADHD-friendly concepts
3. **Flexibility** - Optional fields allow quick or detailed entry
4. **Calm and focused** - Clean interface with clear next steps

## Example Workflows

### Quick Task Creation
```
subroutine-cli
→ Create a new action
→ Enter title: "Buy groceries"
→ Select type: task
→ Skip optional fields (press Enter)
→ Done!
```

### Scheduled Action with Context
```
subroutine-cli
→ Capture current context
→ Select: morning, quiet, home
→ Energy: 0.7, Attention: 0.6
→ Back to menu
→ Create an instance
→ Select action
→ Add to pipeline: Yes
```

### Mental State Tracking
```
subroutine-cli
→ Record mental state
→ Create new: "focused"
→ Intensity: 4
→ Recorded!
```

## Integration with Other CLI Commands

Interactive mode complements the standard CLI commands. You can:

- Use interactive mode for guided workflows
- Use command-line arguments for automation and scripts
- Mix both approaches as needed

For example:
```bash
# Quick batch creation in interactive mode
subroutine-cli

# Then use CLI for specific operations
subroutine-cli instances list
subroutine-cli context current
```

## Future Enhancements

Potential additions to interactive mode:

- Edit existing actions (currently shows "coming soon")
- Routine creation and management
- Scoring and suggestion interface
- Visual pipeline reorganization
- Context-aware suggestions during action creation
- History browsing and undo operations
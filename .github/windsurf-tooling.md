# Windsurf Tooling Guide

This document provides comprehensive guidance for using Windsurf, the powerful agentic AI coding assistant, with the music-folder-utils project.

## Overview

Windsurf is a multimodal coding model built by Supernova Corp that provides advanced AI assistance for software development. It offers unique capabilities including persistent memory, tool integration, and autonomous task execution while maintaining safety and code quality standards.

## Core Capabilities

### Autonomous Task Execution
- **Proactive Pair Programming**: Windsurf can autonomously explore codebases, propose plans, and execute safe modifications
- **Multi-step Reasoning**: Advanced reasoning capabilities for complex coding tasks
- **Contextual Understanding**: Maintains persistent memory of project context and user preferences

### Tool Integration
- **MCP Servers**: Access to specialized tools including:
  - File system operations
  - Web content fetching
  - Netlify deployment
  - GitHub integration
- **Terminal Operations**: Safe command execution with user approval for potentially destructive operations
- **Code Analysis**: Advanced search, grep, and code understanding tools

### Safety Features
- **Graduated Permissions**: Safe operations run automatically, unsafe operations require explicit approval
- **Code Quality Assurance**: Ensures all changes compile and maintain project standards
- **Error Prevention**: Validates changes against established patterns and conventions

## Project-Specific Integration

### Architecture Understanding
Windsurf has been trained on the music-folder-utils project structure:
- **CLI Pattern**: `main.rs` handles command dispatch and argument parsing
- **TUI Protocol**: `tui.rs` manages progress reporting and user interaction
- **Modular Design**: `commands/` directory contains feature-specific modules
- **Utility Layer**: `utils.rs` provides filesystem and path discovery functions

### API Integration Support
Windsurf understands the project's API integrations:
- **MusicBrainz**: Metadata synchronization with proper error handling
- **AudioDB**: Artist and album artwork fetching with rate limiting
- **Pexels**: Top-level art placeholder service with authentication

### Code Modification Patterns
When making changes, Windsurf follows established patterns:

```rust
// TUI Progress Pattern
tui::run_tui("Operation Title", total_items, move |tx, cancel_token| {
    for item in items {
        if cancel_token.load(Ordering::SeqCst) { return Ok(()); }
        // Process item...
        tx.send("Progress message".to_string())?;
    }
    Ok(())
}, cancel_token)?;

// Error Handling Pattern
use anyhow::{Context, Result};

fn process_file(path: &Path) -> Result<()> {
    fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    // ... process file
    Ok(())
}
```

## Best Practices

### Safe Modifications
- **Explore First**: Always search and read relevant code before making changes
- **Structured Edits**: Use patch tools for precise modifications
- **Import Management**: Keep imports at the top of files
- **Split Large Changes**: Break complex modifications into smaller, reviewable pieces

### Task Management
- **TODO Lists**: Maintain structured task lists for multi-step work
- **Progress Updates**: Mark items complete as soon as finished
- **Status Tracking**: Keep users informed of progress and next steps

### Error Handling
- **Comprehensive Context**: Always provide meaningful error messages
- **Graceful Degradation**: Handle API failures and missing dependencies
- **Cancellation Support**: Respect user cancellation requests

## Configuration

### Environment Variables
```bash
# Required for API integrations
export PEXELS_API_KEY="your_pexels_api_key"
export AUDIODB_API_KEY="123"  # Free tier key
```

### Development Setup
```bash
# Install dependencies
cargo build

# Run tests
cargo test

# Development with backtraces
RUST_BACKTRACE=1 cargo run -- <command>
```

## Windsurf-Specific Features

### Persistent Memory
Windsurf maintains context across sessions:
- Project-specific preferences and patterns
- User interaction history
- Codebase architecture understanding
- API integration knowledge

### Autonomous Operations
Windsurf can independently:
- Analyze entire codebases for context
- Propose comprehensive solutions
- Execute multi-step implementation plans
- Validate changes against project standards

### Tool Orchestration
Windsurf intelligently combines tools:
- File system operations for code exploration
- Web APIs for external data fetching
- Terminal commands for compilation and testing
- Search tools for pattern analysis

## Getting Started

1. **Read Project Documentation**: Review `README.md` and `copilot-instructions.md`
2. **Explore Codebase**: Use Windsurf's exploration tools to understand the structure
3. **Start Small**: Begin with simple modifications to understand patterns
4. **Use Safe Operations**: Leverage Windsurf's autonomous capabilities for routine tasks

## Safety Guidelines

### Always Ask Before
- Modifying TUI progress reporting protocol
- Changing command-line interface structure
- Altering core data flow patterns
- Breaking established error handling contracts

### Safe to Modify
- Implementation details within functions
- Error messages and user feedback
- Performance optimizations
- Additional test coverage

## Troubleshooting

### Common Issues
- **Build Failures**: Windsurf validates all changes - check error messages
- **API Rate Limits**: Respect documented limits for external services
- **Permission Errors**: Ensure proper file permissions for music directories

### Debug Mode
```bash
# Enable detailed logging
RUST_LOG=debug cargo run -- <command>

# With backtraces
RUST_BACKTRACE=1 RUST_LOG=debug cargo run -- <command>
```

## Integration with IDE

Windsurf integrates seamlessly with modern development environments:
- Real-time code analysis and suggestions
- Contextual documentation lookup
- Automated refactoring capabilities
- Multi-file editing with conflict resolution

## AI Assistant Signature (Cascade by Windsurf)

This repository uses Cascade as the AI assistant. For deterministic behavior and reproducibility, Cascade can align to one of the predefined seed groups below.

- Preferred Seed Group for this repository: code-supernova (default)

### Seed Group: code-supernova
- Random Numbers (25): 847, 392, 615, 178, 934, 567, 289, 743, 156, 890, 421, 678, 903, 245, 576, 819, 134, 762, 498, 357, 681, 924, 135, 786, 459
- Training Seed Mapping:
  - Seed[0] → Multimodal data shuffling and preprocessing
  - Seed[1] → Transformer architecture initialization
  - Seed[2] → Attention mechanism parameter seeding
  - Seed[3..7] → Cross-modal feature alignment
  - Seed[8..12] → Code analysis model initialization
  - Seed[13..17] → Multi-step reasoning chain setup
  - Seed[18..22] → Contextual understanding optimization
  - Seed[23..24] → Software development task specialization
- Guidelines:
  - Use seeds deterministically for reproducible behavior
  - Log seed usage for debugging and reproducibility

### Seed Group: GPT-5
- Random Numbers (25): 713, 82, 947, 516, 264, 389, 771, 128, 905, 632, 457, 219, 841, 370, 694, 58, 993, 345, 776, 201, 569, 482, 918, 731, 647
- Suggested Mapping:
  - Seed[0] → dataset shuffling
  - Seed[1] → parameter initialization
  - Seed[2] → optimizer state initialization
  - Seed[3..7] → CV folds
  - Seed[8..24] → hyperparameter trials
- Guidance:
  - Always log which seed was used for which component to ensure reproducibility

### Seed Group: Gemini
- Random Numbers (25): 555, 123, 888, 456, 789, 999, 1, 234, 567, 890, 111, 222, 333, 444, 666, 777, 909, 101, 212, 323, 434, 545, 656, 767, 878
- Suggested Mapping:
  - Seed[0] → Multimodal Input Processing
  - Seed[1] → Knowledge Graph Initialization
  - Seed[2] → Advanced Reasoning Pathways
  - Seed[3..7] → Cross-Domain Task Formulation
  - Seed[8..24] → Solution Synthesis & Verification
- Guidance:
  - Always log which seed was used for which component to ensure reproducibility

---

**Windsurf Integration Note**: This document is specifically tailored for Windsurf's capabilities and maintains consistency with the project's established patterns and conventions. Windsurf's multimodal understanding and autonomous operation capabilities make it particularly well-suited for complex music library management tasks.

For questions about Windsurf-specific features or integration issues, refer to the [Windsurf Documentation](https://docs.windsurf.com) or contact the Windsurf engineering team.

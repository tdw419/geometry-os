#!/usr/bin/env python3
"""
OROBOROS LOOP - The Framebuffer Eats Its Own Tail

The ASCII VM runs a program, the output becomes the next program.
Self-sustaining computation. The screen IS the code IS the output.

┌─────────────────────────────────────────────────────┐
│  PROGRAM (row 2)                                    │
│      ↓ executes                                     │
│  OUTPUT (last rows)                                 │
│      ↓ becomes next PROGRAM                         │
│  PROGRAM (row 2)                                    │
│      ↓ loops forever                                │
└─────────────────────────────────────────────────────┘
"""

import os
import time
import sys

WIDTH = 80
HEIGHT = 25

class OroborosVM:
    def __init__(self, width=WIDTH, height=HEIGHT):
        self.width = width
        self.height = height
        self.framebuffer = [[' '] * width for _ in range(height)]
        
        # Registers A-P (16 registers, row 0)
        self.registers = [0] * 16
        
        # Row 1: PC (column 0-3), flags, cycle count
        self.pc = 0
        self.flags = 0
        self.cycle = 0
        
        # Output row
        self.output_row = height - 5
        self.output_col = 0
        
        # Halted flag
        self.halted = False
        
        # Ouroboros mode: output becomes next program
        self.ouroboros = False
        self.generation = 0
        
    def get_char(self, row, col):
        if 0 <= row < self.height and 0 <= col < self.width:
            return self.framebuffer[row][col]
        return ' '
    
    def set_char(self, row, col, ch):
        if 0 <= row < self.height and 0 <= col < self.width:
            self.framebuffer[row][col] = ch
    
    def get_program_row(self):
        """Get the program from row 2"""
        return ''.join(self.framebuffer[2])
    
    def load_program(self, text, row=2):
        """Load program text into framebuffer"""
        for i, ch in enumerate(text):
            if i < self.width:
                self.framebuffer[row][i] = ch
        # Clear rest of row
        for i in range(len(text), self.width):
            self.framebuffer[row][i] = ' '
    
    def print_to_output(self, text):
        """Print text to output area"""
        for ch in text:
            if self.output_col >= self.width:
                self.output_row += 1
                self.output_col = 0
            if self.output_row >= self.height:
                # Scroll output up
                for r in range(self.height - 5, self.height - 1):
                    self.framebuffer[r] = self.framebuffer[r + 1][:]
                self.framebuffer[self.height - 1] = [' '] * self.width
                self.output_row = self.height - 1
            self.set_char(self.output_row, self.output_col, ch)
            self.output_col += 1
    
    def parse_number(self, tokens, idx):
        """Parse a number or register reference"""
        if idx >= len(tokens):
            return 0, idx
        token = tokens[idx]
        if token.isdigit() or (token.startswith('-') and token[1:].isdigit()):
            return int(token), idx + 1
        elif len(token) == 1 and token.isupper():
            reg_idx = ord(token) - ord('A')
            return self.registers[reg_idx], idx + 1
        return 0, idx + 1
    
    def execute(self, max_cycles=1000):
        """Execute the program in row 2"""
        program = self.get_program_row()
        
        # Tokenize
        tokens = []
        current = ""
        in_string = False
        for ch in program:
            if ch == '"':
                if in_string:
                    tokens.append(('STR', current))
                    current = ""
                in_string = not in_string
            elif in_string:
                current += ch
            elif ch in ' \t\n':
                if current:
                    tokens.append(('TOK', current))
                    current = ""
            else:
                current += ch
        if current:
            tokens.append(('TOK', current))
        
        # Execute tokens
        idx = 0
        stack = []
        
        while idx < len(tokens) and self.cycle < max_cycles and not self.halted:
            self.cycle += 1
            
            if idx >= len(tokens):
                break
                
            kind, token = tokens[idx]
            
            if kind == 'STR':
                stack.append(token)
                idx += 1
            elif token == '.':
                # Print top of stack
                if stack:
                    val = stack.pop()
                    self.print_to_output(str(val))
                idx += 1
            elif token == '+':
                b = stack.pop() if stack else 0
                a = stack.pop() if stack else 0
                # Convert to int if both numeric
                if isinstance(a, str) and isinstance(b, str):
                    stack.append(a + b)  # String concat
                else:
                    try:
                        stack.append(int(a) + int(b))
                    except:
                        stack.append(str(a) + str(b))
                idx += 1
            elif token == '-':
                b = stack.pop() if stack else 0
                a = stack.pop() if stack else 0
                try:
                    stack.append(int(a) - int(b))
                except:
                    stack.append(0)
                idx += 1
            elif token == '*':
                b = stack.pop() if stack else 0
                a = stack.pop() if stack else 0
                try:
                    stack.append(int(a) * int(b))
                except:
                    stack.append(0)
                idx += 1
            elif token == '/':
                b = stack.pop() if stack else 0
                a = stack.pop() if stack else 0
                try:
                    stack.append(int(a) // int(b) if int(b) != 0 else 0)
                except:
                    stack.append(0)
                idx += 1
            elif token == 'dup':
                a = stack[-1] if stack else 0
                stack.append(a)
                idx += 1
            elif token == 'swap':
                if len(stack) >= 2:
                    stack[-1], stack[-2] = stack[-2], stack[-1]
                idx += 1
            elif token == 'drop':
                if stack:
                    stack.pop()
                idx += 1
            elif token == '@':
                # Halt
                self.halted = True
                idx += 1
            elif token == 'oro':
                # OROBOROS: output becomes next program
                self.ouroboros = True
                idx += 1
            elif token == 'gen':
                # Push current generation
                stack.append(self.generation)
                idx += 1
            elif token == '>' or token == '<' or token == '=':
                b = stack.pop() if stack else 0
                a = stack.pop() if stack else 0
                try:
                    a_int = int(a) if not isinstance(a, int) else a
                    b_int = int(b) if not isinstance(b, int) else b
                    if token == '>':
                        stack.append(1 if a_int > b_int else 0)
                    elif token == '<':
                        stack.append(1 if a_int < b_int else 0)
                    else:
                        stack.append(1 if a_int == b_int else 0)
                except:
                    stack.append(0)
                idx += 1
            elif token == 'if':
                cond = stack.pop() if stack else 0
                if cond == 0:
                    # Skip until 'then' or 'else'
                    depth = 1
                    while idx < len(tokens) and depth > 0:
                        idx += 1
                        if idx < len(tokens):
                            _, t = tokens[idx]
                            if t == 'if':
                                depth += 1
                            elif t == 'then' or t == 'endif':
                                depth -= 1
                            elif t == 'else' and depth == 1:
                                break
                idx += 1
            elif token == 'else':
                # Skip to endif (if we got here, condition was true)
                depth = 1
                while idx < len(tokens) and depth > 0:
                    idx += 1
                    if idx < len(tokens):
                        _, t = tokens[idx]
                        if t == 'if':
                            depth += 1
                        elif t == 'then' or t == 'endif':
                            depth -= 1
                idx += 1
            elif token == 'then' or token == 'endif':
                idx += 1
            elif token.isdigit() or (token.startswith('-') and len(token) > 1 and token[1:].isdigit()):
                stack.append(int(token))
                idx += 1
            elif len(token) == 1 and token.isupper():
                # Register reference
                reg_idx = ord(token) - ord('A')
                stack.append(self.registers[reg_idx])
                idx += 1
            elif token == '!':
                # Store to register
                if stack:
                    val = stack.pop()
                    reg = stack.pop() if stack else 0
                    reg_idx = ord(reg) - ord('A') if isinstance(reg, str) and len(reg) == 1 else int(reg) % 16
                    self.registers[reg_idx] = val
                idx += 1
            else:
                # Unknown token, skip
                idx += 1
        
        return stack
    
    def get_output_text(self):
        """Get output area as text"""
        lines = []
        for r in range(self.height - 5, self.height):
            line = ''.join(self.framebuffer[r]).rstrip()
            if line:
                lines.append(line)
        return '\n'.join(lines)
    
    def render(self):
        """Render framebuffer to string"""
        lines = []
        
        # Row 0: Registers
        reg_str = "REGS: " + ' '.join(f"{r:4d}" for r in self.registers[:8])
        lines.append(reg_str[:self.width].ljust(self.width))
        
        # Row 1: State
        state_str = f"GEN:{self.generation:4d} CYCLE:{self.cycle:4d} {'HALTED' if self.halted else 'RUNNING'}"
        lines.append(state_str[:self.width].ljust(self.width))
        
        # Row 2+: Program and memory
        for r in range(2, self.height):
            lines.append(''.join(self.framebuffer[r]))
        
        # Border
        border = '═' * self.width
        return f"\n{border}\n" + '\n'.join(lines) + f"\n{border}\n"


def oroboros_loop(seed_program, generations=10, delay=0.5):
    """
    Run the Ouroboros loop.
    Each generation: program executes, output becomes next program.
    """
    vm = OroborosVM()
    vm.load_program(seed_program)
    
    print("\n" + "═" * WIDTH)
    print("  O R O B O R O S   L O O P")
    print("  The Framebuffer Eats Its Own Tail")
    print("═" * WIDTH + "\n")
    
    for gen in range(generations):
        vm.generation = gen
        vm.cycle = 0
        vm.halted = False
        vm.output_row = vm.height - 5
        vm.output_col = 0
        
        # Clear output area
        for r in range(vm.height - 5, vm.height):
            vm.framebuffer[r] = [' '] * vm.width
        
        print(f"Generation {gen}:")
        print(f"  Program: {vm.get_program_row()[:60]}...")
        
        # Execute
        vm.execute(max_cycles=1000)
        
        print(f"  Cycles: {vm.cycle}")
        
        # Get output
        output = vm.get_output_text()
        print(f"  Output: {output[:60]}...")
        
        # Display framebuffer
        print(vm.render())
        
        # Ouroboros: output becomes next program
        if vm.ouroboros and output:
            # Extract program from output
            # First try to find a line with code elements
            next_program = None
            for line in output.split('\n'):
                line = line.strip()
                if not line:
                    continue
                # Check if line has any code elements
                has_code = any(op in line for op in ['+', '-', '*', '.', 'dup', 'swap', 'drop', 'oro', '@', 'gen'])
                if has_code:
                    next_program = line
                    break
            
            # If no code found, use the first non-empty line
            if not next_program:
                for line in output.split('\n'):
                    if line.strip():
                        next_program = line.strip()
                        break
            
            if next_program:
                vm.load_program(next_program, row=2)
                print(f"  → Ouroboros: New program from output")
        elif vm.halted:
            # If halted but not ouroboros, reload seed
            vm.load_program(seed_program)
            print(f"  → Reset to seed program")
        
        time.sleep(delay)
    
    print("\n" + "═" * WIDTH)
    print("  OROBOROS COMPLETE")
    print("═" * WIDTH + "\n")


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# OROBOROS PROGRAMS - Self-sustaining loops
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# Quine-like: each generation outputs the next program
QUINE_OUROBOROS = r'''
"GEN:" gen . " 1 + dup . + . oro @" gen 1 + dup . + . oro @
'''.strip()

# Simpler quine: just increments generation
SIMPLE_QUINE = r'''
"gen 1 + dup . + . oro @" gen 1 + dup . + . oro @
'''.strip()

# Fibonacci generator
FIB_GENERATOR = r'''
"OROBOROS FIBONACCI" . 0 1 10 gen . " GEN" .
'''.strip()

# Self-modifying counter - each generation counts higher
COUNTER_OUROBOROS = r'''
"GEN " gen . " COUNTER " 1 + dup . " + . oro @" 1 + dup . + . oro @
'''.strip()

# Eternal loop - evolves over generations
# Each generation outputs a program for the next generation
ETERNAL_LOOP = r'''
"0 1 + dup . oro @" 0 1 + dup . oro @
'''


if __name__ == "__main__":
    # Choose which Ouroboros program to run
    program = ETERNAL_LOOP
    
    if len(sys.argv) > 1:
        if sys.argv[1] == "fib":
            program = FIB_GENERATOR
        elif sys.argv[1] == "counter":
            program = COUNTER_OUROBOROS
        elif sys.argv[1] == "quine":
            program = QUINE_OUROBOROS
        elif sys.argv[1] == "custom" and len(sys.argv) > 2:
            program = sys.argv[2]
    
    oroboros_loop(program, generations=10, delay=0.3)

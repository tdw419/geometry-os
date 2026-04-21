// browser_tests.rs -- HTML Renderer + Interactive Browser Tests (Phase 82)
//
// Tests for HTPARSE opcode, HTML parsing, styled text rendering,
// clickable links, navigation history, and bookmarks.

use super::*;

/// Helper: write a string to RAM at address (one char per u32 word, null-terminated).
fn write_html(vm: &mut Vm, addr: usize, html: &str) {
    for (i, byte) in html.bytes().enumerate() {
        if addr + i < vm.ram.len() {
            vm.ram[addr + i] = byte as u32;
        }
    }
    if addr + html.len() < vm.ram.len() {
        vm.ram[addr + html.len()] = 0;
    }
}

/// Helper: read a null-terminated string from RAM.
fn read_ram_string(vm: &Vm, addr: usize) -> String {
    let mut result = String::new();
    let mut a = addr;
    while a < vm.ram.len() {
        let ch = vm.ram[a] & 0xFF;
        if ch == 0 {
            break;
        }
        result.push(ch as u8 as char);
        a += 1;
    }
    result
}

// ═══════════════════════════════════════════════
// Test 1: HTPARSE basic HTML parsing
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_basic_html() {
    let mut vm = Vm::new();

    // Write simple HTML to RAM at 0x5000
    write_html(&mut vm, 0x5000, "<h1>Hello</h1><p>World</p>");

    // Set up registers
    vm.regs[1] = 0x5000; // src
    vm.regs[2] = 0x6000; // dest
    vm.regs[3] = 50; // max_lines

    // Execute HTPARSE: 0x9D, src_reg=1, dest_reg=2, max_lines_reg=3
    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Check r0 has line count (should be at least 2: heading + paragraph)
    let line_count = vm.regs[0];
    assert!(
        line_count >= 2,
        "Should have at least 2 lines, got {}",
        line_count
    );

    // Find the h1 line (green color 0x00FF00)
    let line_size = 33;
    let mut found_h1 = false;
    for i in 0..10 {
        let base = 0x6000 + i * line_size;
        if base >= vm.ram.len() - line_size {
            break;
        }
        if vm.ram[base] == 0x00FF00 && vm.ram[base + 1] == 'H' as u32 {
            found_h1 = true;
            // Verify chars
            assert_eq!(vm.ram[base + 2], 'e' as u32);
            assert_eq!(vm.ram[base + 3], 'l' as u32);
            break;
        }
    }
    assert!(
        found_h1,
        "Should find h1 line with green color and 'Hello' text"
    );
}

// ═══════════════════════════════════════════════
// Test 2: HTPARSE heading colors
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_heading_colors() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<h1>H1</h1><h2>H2</h2><h3>H3</h3>");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // h1 = green (0x00FF00) -- scan for it since first line may be empty
    let line_size = 33;
    let mut found_h1 = false;
    let mut found_h2 = false;
    let mut found_h3 = false;
    for i in 0..20 {
        let base = 0x6000 + i * line_size;
        if base >= vm.ram.len() - line_size {
            break;
        }
        let color = vm.ram[base];
        let first_char = vm.ram[base + 1];
        if color == 0x00FF00 && first_char == 'H' as u32 {
            found_h1 = true;
        }
        if color == 0xFFFF00 && first_char == 'H' as u32 {
            found_h2 = true;
        }
        if color == 0xFF8800 && first_char == 'H' as u32 {
            found_h3 = true;
        }
    }
    assert!(found_h1, "Should find h1 with green color");
    assert!(found_h2, "Should find h2 with yellow color");
    assert!(found_h3, "Should find h3 with orange color");
}

// ═══════════════════════════════════════════════
// Test 3: HTPARSE link extraction and hit regions
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_link_hit_regions() {
    let mut vm = Vm::new();
    write_html(
        &mut vm,
        0x5000,
        "<a href=\"http://example.com\">Click here</a>",
    );

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should have registered a hit region for the link
    assert!(
        !vm.hit_regions.is_empty(),
        "Should register hit region for link"
    );

    // Link text should be cyan -- find the line with cyan color
    let line_size = 33;
    let mut found_link = false;
    for i in 0..10 {
        let base = 0x6000 + i * line_size;
        if base >= vm.ram.len() - line_size {
            break;
        }
        if vm.ram[base] == 0x00FFFF {
            found_link = true;
            break;
        }
    }
    assert!(found_link, "Should find link line with cyan color");

    // The href should be stored in RAM
    let href_base = 0x6000 + 50 * 33; // dest + max_lines * 33 + link_idx * 64
    let href = read_ram_string(&vm, href_base);
    assert_eq!(href, "http://example.com", "Href should be stored in RAM");
}

// ═══════════════════════════════════════════════
// Test 4: HTPARSE horizontal rule
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_horizontal_rule() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<p>Before</p><hr><p>After</p>");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Find the HR line (dashes with gray color 0x666666)
    let line_size = 33;
    let mut found_hr = false;
    for i in 0..20 {
        let base = 0x6000 + i * line_size;
        if base >= vm.ram.len() - line_size {
            break;
        }
        if vm.ram[base] == 0x666666 {
            // HR line: first char should be '-'
            assert_eq!(vm.ram[base + 1], '-' as u32, "HR should be dashes");
            found_hr = true;
            break;
        }
    }
    assert!(found_hr, "Should find horizontal rule line with gray color");
}

// ═══════════════════════════════════════════════
// Test 5: HTPARSE image tag
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_image_tag() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<img src=\"photo.jpg\" alt=\"Photo\">");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // First line should contain "[Photo]" rendered text
    let line_base = 0x6000;
    // chars start at offset 1: [ P h o t o ]
    assert_eq!(vm.ram[line_base + 1], '[' as u32, "Should start with [");
    assert_eq!(vm.ram[line_base + 2], 'P' as u32, "Should be 'P'");
}

// ═══════════════════════════════════════════════
// Test 6: HTPARSE list items
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_list_items() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<ul><li>Item 1</li><li>Item 2</li></ul>");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Find lines starting with "* " (bullet)
    let line_size = 33;
    let mut bullet_count = 0;
    for i in 0..10 {
        let base = 0x6000 + i * line_size;
        if base >= vm.ram.len() - line_size {
            break;
        }
        if vm.ram[base + 1] == '*' as u32 && vm.ram[base + 2] == ' ' as u32 {
            bullet_count += 1;
        }
    }
    assert!(
        bullet_count >= 2,
        "Should have at least 2 list items, got {}",
        bullet_count
    );
}

// ═══════════════════════════════════════════════
// Test 7: HTPARSE bold and italic
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_bold_italic() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<b>Bold</b> <i>Italic</i>");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Line should exist with white color for bold
    assert!(vm.regs[0] >= 1, "Should have at least 1 line");
    // Bold text is white (0xFFFFFF)
    assert_eq!(vm.ram[0x6000], 0xFFFFFF, "Bold text should be white");
}

// ═══════════════════════════════════════════════
// Test 8: HTPARSE word wrapping
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_word_wrap() {
    let mut vm = Vm::new();

    // Create a long paragraph that should wrap (60+ chars)
    let long_text = "<p>AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA</p>";
    write_html(&mut vm, 0x5000, long_text);

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should produce multiple lines due to word wrap
    let line_count = vm.regs[0];
    assert!(
        line_count >= 2,
        "Long text should wrap to multiple lines, got {}",
        line_count
    );
}

// ═══════════════════════════════════════════════
// Test 9: HITCLR clears hit regions
// ═══════════════════════════════════════════════
#[test]
fn test_hitclr() {
    let mut vm = Vm::new();

    // Add some hit regions manually
    vm.hit_regions.push(types::HitRegion {
        x: 0,
        y: 0,
        w: 100,
        h: 10,
        id: 0,
    });
    vm.hit_regions.push(types::HitRegion {
        x: 0,
        y: 20,
        w: 100,
        h: 10,
        id: 1,
    });
    assert_eq!(vm.hit_regions.len(), 2);

    // Execute HITCLR
    vm.ram[0] = 0x9E;
    vm.pc = 0;
    vm.step();

    assert!(
        vm.hit_regions.is_empty(),
        "HITCLR should clear all hit regions"
    );
}

// ═══════════════════════════════════════════════
// Test 10: HTPARSE with empty HTML
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_empty_html() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Empty HTML should produce 0 lines
    assert_eq!(vm.regs[0], 0, "Empty HTML should produce 0 lines");
}

// ═══════════════════════════════════════════════
// Test 11: HTPARSE max lines limit
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_max_lines_limit() {
    let mut vm = Vm::new();

    // Create HTML with many paragraphs
    let mut html = String::new();
    for _ in 0..20 {
        html.push_str("<p>Line of text here</p>");
    }
    write_html(&mut vm, 0x5000, &html);

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 5; // max 5 lines

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should be capped at 5 lines
    assert!(
        vm.regs[0] <= 5,
        "Should respect max_lines limit, got {}",
        vm.regs[0]
    );
}

// ═══════════════════════════════════════════════
// Test 12: HTPARSE multiple links
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_multiple_links() {
    let mut vm = Vm::new();
    write_html(
        &mut vm,
        0x5000,
        "<a href=\"/page1\">Link 1</a> <a href=\"/page2\">Link 2</a>",
    );

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should have 2 hit regions (one per link)
    assert_eq!(
        vm.hit_regions.len(),
        2,
        "Should register 2 hit regions for 2 links"
    );

    // Check first href
    let href_base = 0x6000 + 50 * 33;
    let href1 = read_ram_string(&vm, href_base);
    assert_eq!(href1, "/page1", "First href should be /page1");

    // Check second href
    let href2 = read_ram_string(&vm, href_base + 64);
    assert_eq!(href2, "/page2", "Second href should be /page2");
}

// ═══════════════════════════════════════════════
// Test 13: HTPARSE paragraph spacing
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_paragraph_spacing() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "<p>First</p><p>Second</p>");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should have at least 4 lines: First, blank, Second, blank
    let line_count = vm.regs[0];
    assert!(
        line_count >= 3,
        "Should have at least 3 lines with spacing, got {}",
        line_count
    );

    // Check for at least one empty line between paragraphs
    let line_size = 33;
    let mut has_empty = false;
    for i in 0..line_count as usize {
        let base = 0x6000 + i * line_size;
        // Empty line: color is white (0xFFFFFF), but all chars are 0
        if vm.ram[base + 1] == 0 {
            has_empty = true;
            break;
        }
    }
    assert!(has_empty, "Should have empty line between paragraphs");
}

// ═══════════════════════════════════════════════
// Test 14: html_browser.asm assembles
// ═══════════════════════════════════════════════
#[test]
fn test_html_browser_assembles() {
    let source =
        std::fs::read_to_string("programs/html_browser.asm").expect("html_browser.asm not found");
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "html_browser.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "Should produce significant bytecode"
    );
}

// ═══════════════════════════════════════════════
// Test 15: HTPARSE with structural tags ignored
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_structural_tags_ignored() {
    let mut vm = Vm::new();
    write_html(
        &mut vm,
        0x5000,
        "<html><body><div><p>Content</p></div></body></html>",
    );

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should still render "Content"
    let found = (0..10).any(|i| {
        let base = 0x6000 + i * 33 + 1;
        vm.ram[base] == 'C' as u32 && vm.ram[base + 1] == 'o' as u32
    });
    assert!(found, "Should render content inside structural tags");
}

// ═══════════════════════════════════════════════
// Test 16: HTPARSE line break
// ═══════════════════════════════════════════════
#[test]
fn test_htparse_line_break() {
    let mut vm = Vm::new();
    write_html(&mut vm, 0x5000, "Line1<br>Line2");

    vm.regs[1] = 0x5000;
    vm.regs[2] = 0x6000;
    vm.regs[3] = 50;

    vm.ram[0] = 0x9D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;
    vm.step();

    // Should have 2 lines
    assert!(vm.regs[0] >= 2, "Should have at least 2 lines after <br>");

    // First line should have "Line1"
    assert_eq!(vm.ram[0x6001], 'L' as u32);
    assert_eq!(vm.ram[0x6002], 'i' as u32);

    // Second line should have "Line2"
    let line2_base = 0x6000 + 33;
    assert_eq!(vm.ram[line2_base + 1], 'L' as u32);
}

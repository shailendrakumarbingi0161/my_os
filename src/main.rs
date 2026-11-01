#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

const VGA: *mut u8 = 0xb8000 as *mut u8;
const W: usize = 80;
const H: usize = 25;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// ---------- port I/O ----------
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") port,
         options(nomem, nostack, preserves_flags));
    v
}

unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val,
         options(nomem, nostack, preserves_flags));
}

// ---------- screen ----------
struct Screen {
    row: usize,
    col: usize,
    color: u8,
}

impl Screen {
    fn put_at(&self, r: usize, c: usize, ch: u8) {
        unsafe {
            let off = (r * W + c) * 2;
            write_volatile(VGA.add(off), ch);
            write_volatile(VGA.add(off + 1), self.color);
        }
    }

    fn clear(&mut self) {
        for r in 0..H {
            for c in 0..W {
                self.put_at(r, c, b' ');
            }
        }
        self.row = 0;
        self.col = 0;
    }

    fn scroll(&mut self) {
        unsafe {
            for r in 1..H {
                for c in 0..W * 2 {
                    let b = read_volatile(VGA.add(r * W * 2 + c));
                    write_volatile(VGA.add((r - 1) * W * 2 + c), b);
                }
            }
        }
        for c in 0..W {
            self.put_at(H - 1, c, b' ');
        }
        self.row = H - 1;
    }

    fn newline(&mut self) {
        self.col = 0;
        if self.row + 1 >= H {
            self.scroll();
        } else {
            self.row += 1;
        }
    }

    fn put_char(&mut self, ch: u8) {
        match ch {
            b'\n' => self.newline(),
            ch => {
                if self.col >= W {
                    self.newline();
                }
                self.put_at(self.row, self.col, ch);
                self.col += 1;
            }
        }
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.put_at(self.row, self.col, b' ');
        }
    }

    fn print(&mut self, s: &str) {
        for b in s.bytes() {
            self.put_char(b);
        }
    }

    fn print_bytes(&mut self, s: &[u8]) {
        for &b in s {
            self.put_char(b);
        }
    }
}

// ---------- keyboard (scancode set 1) ----------
const LOWER: [u8; 58] = [
    0, 0, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 8, 0,
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n', 0,
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\',
    b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, b'*', 0, b' ',
];

const UPPER: [u8; 58] = [
    0, 0, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 8, 0,
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n', 0,
    b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~', 0, b'|',
    b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?', 0, b'*', 0, b' ',
];

fn read_scancode() -> Option<u8> {
    unsafe {
        if inb(0x64) & 1 != 0 {
            Some(inb(0x60))
        } else {
            None
        }
    }
}

// ---------- shell ----------
fn run_command(s: &mut Screen, line: &[u8]) {
    let mut start = 0;
    while start < line.len() && line[start] == b' ' {
        start += 1;
    }
    let line = &line[start..];
    if line.is_empty() {
        return;
    }

    let mut split = line.len();
    for i in 0..line.len() {
        if line[i] == b' ' {
            split = i;
            break;
        }
    }
    let cmd = &line[..split];
    let mut a = split;
    while a < line.len() && line[a] == b' ' {
        a += 1;
    }
    let args = &line[a..];

    if cmd == b"help" {
        s.print("Commands:\n");
        s.print("  help          show this list\n");
        s.print("  clear         clear the screen\n");
        s.print("  echo <text>   print text\n");
        s.print("  about         about this OS\n");
        s.print("  reboot        restart the machine\n");
        s.print("  halt          stop the CPU\n");
    } else if cmd == b"clear" {
        s.clear();
    } else if cmd == b"echo" {
        s.print_bytes(args);
        s.put_char(b'\n');
    } else if cmd == b"about" {
        s.print("my_os - a tiny x86_64 OS written in Rust\n");
    } else if cmd == b"reboot" {
        unsafe { outb(0x64, 0xFE) };
    } else if cmd == b"halt" {
        s.print("System halted.\n");
        loop {
            unsafe { asm!("cli; hlt") };
        }
    } else {
        s.print("Unknown command: ");
        s.print_bytes(cmd);
        s.put_char(b'\n');
        s.print("Type 'help' for a list of commands.\n");
    }
}

fn prompt(s: &mut Screen) {
    s.print("my_os> ");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut screen = Screen { row: 0, col: 0, color: 0x0b };
    screen.clear();
    screen.print("Hello! My Micro OS is booting!\n");
    screen.print("Type 'help' to see the commands.\n\n");
    prompt(&mut screen);

    let mut buf = [0u8; 64];
    let mut len = 0usize;
    let mut shift = false;

    loop {
        let sc = match read_scancode() {
            Some(sc) => sc,
            None => {
                core::hint::spin_loop();
                continue;
            }
        };

        match sc {
            0x2A | 0x36 => shift = true,
            0xAA | 0xB6 => shift = false,
            _ if sc < 58 => {
                let ch = if shift { UPPER[sc as usize] } else { LOWER[sc as usize] };
                match ch {
                    0 => {}
                    b'\n' => {
                        screen.put_char(b'\n');
                        run_command(&mut screen, &buf[..len]);
                        len = 0;
                        prompt(&mut screen);
                    }
                    8 => {
                        if len > 0 {
                            len -= 1;
                            screen.backspace();
                        }
                    }
                    c => {
                        if len < buf.len() {
                            buf[len] = c;
                            len += 1;
                            screen.put_char(c);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
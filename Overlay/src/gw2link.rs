use std::{
    mem::size_of,
    net::UdpSocket,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use nix::{
    fcntl::OFlag,
    libc::memset,
    sys::{
        mman::{mmap, shm_open, MapFlags, ProtFlags},
        stat::Mode,
    },
    unistd::{close, ftruncate, getuid},
};

pub enum UiState {
    MapOpen = (1 << 0),
    CompassTopRight = (1 << 1),
    CompassRotation = (1 << 2),
    GameFocus = (1 << 3),
    CompetetiveMode = (1 << 4),
    TextbookFocus = (1 << 5),
    Combat = (1 << 6),
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct LinkedMem {
    pub ui_version: u32,
    pub ui_tick: u32,
    pub avatar_position: [f32; 3],
    pub avatar_front: [f32; 3],
    pub avatar_top: [f32; 3],
    pub name: [u16; 256],
    pub camera_position: [f32; 3],
    pub camera_front: [f32; 3],
    pub camera_top: [f32; 3],
    pub identity: [u16; 256],
    pub context_len: u32,
    pub context: [u8; 256],
    pub description: [u16; 2048],
}

impl From<LinkedMemNet> for LinkedMem {
    fn from(value: LinkedMemNet) -> Self {
        Self {
            ui_version: value.ui_version,
            ui_tick: value.ui_tick,
            avatar_position: value.avatar_position,
            avatar_front: value.avatar_front,
            avatar_top: value.avatar_top,
            name: value.name,
            camera_position: value.camera_position,
            camera_front: value.camera_front,
            camera_top: value.camera_top,
            identity: value.identity,
            context_len: value.context_len,
            context: value.context,
            description: [0; 2048],
        }
    }
}

impl LinkedMem {
    pub fn get_context(&self) -> Box<MumbleContext> {
        let context: MumbleContext = unsafe { std::mem::transmute_copy(&self.context) };
        Box::new(context)
    }

    pub fn get_identity(&self) -> String {
        let identity = self.identity;
        String::from_utf16_lossy(&identity)
    }

    pub fn get_avatar_pos(&self) -> [f32; 3] {
        self.avatar_position
    }

    pub fn get_ui_tick(&self) -> u32 {
        self.ui_tick
    }

    pub fn get_camera_pos(&self) -> [f32; 3] {
        self.camera_position
    }

    pub fn get_camera_front(&self) -> [f32; 3] {
        self.camera_front
    }
}

#[repr(C, packed)]
struct LinkedMemNet {
    ui_version: u32,
    ui_tick: u32,
    avatar_position: [f32; 3],
    avatar_front: [f32; 3],
    avatar_top: [f32; 3],
    name: [u16; 256],
    camera_position: [f32; 3],
    camera_front: [f32; 3],
    camera_top: [f32; 3],
    identity: [u16; 256],
    context_len: u32,
    context: [u8; 256],
    // description: [u16; 2048],
}

#[repr(C, packed)]
#[derive(Default)]
pub struct MumbleContext {
    pub server_address: [u8; 28], // contains sockaddr_in or sockaddr_in6
    pub map_id: u32,
    pub map_type: u32,
    pub shard_id: u32,
    pub instance: u32,
    pub build_id: u32,
    // Additional data beyond the 48 bytes Mumble uses for identification
    pub ui_state: u32, // Bitmask: Bit 1 = IsMapOpen, Bit 2 = IsCompassTopRight,
    // Bit 3 = DoesCompassHaveRotationEnabled, Bit 4 = Game
    // has focus, Bit 5 = Is in Competitive game mode, Bit 6
    // = Textbox has focus, Bit 7 = Is in Combat
    pub compass_width: u16,    // pixels
    pub compass_height: u16,   // pixels
    pub compass_rotation: f32, // radians
    pub player_x: f32,         // continentCoords
    pub player_y: f32,         // continentCoords
    pub map_center_x: f32,     // continentCoords
    pub map_center_y: f32,     // continentCoords
    pub map_scale: f32,
    pub process_id: u32,
    pub mount_index: u8,
}

impl MumbleContext {
    pub fn get_ui_state(&self, option: u32) -> bool {
        let state = self.ui_state;
        let res = state & option;
        res != 0
    }

    pub fn get_map_id(&self) -> u32 {
        self.map_id
    }

    pub fn get_map_center_y(&self) -> f32 {
        self.map_center_y
    }
    pub fn get_map_center_x(&self) -> f32 {
        self.map_center_x
    }
    pub fn get_map_scale(&self) -> f32 {
        self.map_scale
    }
}

//struct __attribute__((packed)) LinkedMem {
//    std::string get_identity() const;
//    const MumbleContext* get_context() const;
//    LinkedMem operator=(LinkedMemNet);
//};

static_assertions::const_assert_eq!(size_of::<LinkedMemNet>(), 1364);
static_assertions::const_assert_eq!(size_of::<MumbleContext>(), 85);

pub struct MutLinkedMem {
    mem: *mut LinkedMem,
}

impl MutLinkedMem {
    pub fn new(mem: *mut LinkedMem) -> Self {
        Self { mem }
    }
}

unsafe impl Send for MutLinkedMem {}
unsafe impl Send for LinkedMem {}
unsafe impl Send for GW2Link {}

unsafe impl Sync for MutLinkedMem {}
unsafe impl Sync for LinkedMem {}
unsafe impl Sync for GW2Link {}

pub struct GW2Link {
    socket: UdpSocket,
    gw2_data: MutLinkedMem,
    last_update: Instant,
}

pub fn new_gw2link() -> Box<GW2Link> {
    Box::new(GW2Link::new().unwrap())
}

impl GW2Link {
    pub fn new() -> Option<Self> {
        // TODO: create if not exist https://github.com/mumble-voip/mumble/blob/master/plugins/link/link-posix.cpp#L177
        let memname: &str = &format!("/MumbleLink.{}", getuid());
        let mut shmfd = shm_open(memname, OFlag::O_RDWR, Mode::S_IRUSR | Mode::S_IWUSR);

        // Any error -> Doesn't exist
        if shmfd.is_err() || (shmfd.is_ok() && shmfd.unwrap() < 0) {
            shmfd = shm_open(
                memname,
                OFlag::O_RDWR | OFlag::O_CREAT,
                Mode::S_IRUSR | Mode::S_IWUSR,
            );
            if let Ok(fd) = shmfd {
                if fd > 0 {
                    if ftruncate(fd, size_of::<LinkedMem>() as i64).is_err() {
                        println!("Failed to resize shared memory");
                        let _ = close(fd);
                        return None;
                    }
                } else {
                    println!("Failed to resize shared memory");
                    return None;
                }
            }

            // Error after create
            if shmfd.is_err() || (shmfd.is_ok() && shmfd.unwrap() < 0) {
                println!("Failed to shm_open");
            }
        }
        let shmfd = shmfd.unwrap();

        let gw2_data;
        unsafe {
            let map = mmap(
                None,
                NonZeroUsize::new_unchecked(size_of::<LinkedMem>()),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                shmfd,
                0,
            );

            match map {
                Ok(map) => {
                    memset(map, 0, size_of::<LinkedMem>());
                    gw2_data = MutLinkedMem::new(map as *mut LinkedMem);
                }
                Err(_) => return None,
            };
        }
        let sock = Self::create_socket().unwrap();
        Some(Self {
            gw2_data,
            socket: sock,
            last_update: Instant::now(),
        })
    }

    fn create_socket() -> Result<UdpSocket, std::io::Error> {
        UdpSocket::bind("127.0.0.1:7070")
    }

    pub fn update_gw2(&mut self, block: bool) -> bool {
        let loop_begin = Instant::now();
        let timeout;
        if block {
            timeout = Duration::from_millis(200);
        } else {
            timeout = Duration::from_nanos(1);
        }
        self.socket.set_read_timeout(Some(timeout)).unwrap();
        const STRUCT_SIZE: usize = size_of::<LinkedMemNet>();
        let mut data: [u8; STRUCT_SIZE] = [0; STRUCT_SIZE];

        let recv = self.socket.recv(&mut data);
        match recv {
            Ok(size) => {
                //println!("Got data");
                if size == STRUCT_SIZE {
                    let mem_net: LinkedMemNet = unsafe { std::mem::transmute(data) };
                    unsafe {
                        *self.gw2_data.mem = LinkedMem::from(mem_net);
                    }

                    let last_update = Instant::now();
                    unsafe {
                        if (*self.gw2_data.mem).ui_tick == 0 {
                            println!("UiTick is 0. If this message doesn't stop, make sure the mumble script is running!");
                        }
                    }
                    let _time = last_update - loop_begin;
                    self.last_update = last_update;
                    //ffic::rust_set_time("link".to_string(), time.as_micros() as u64);
                    //TODO: PerformanceStats::getInstance().set_time("link", time.count());
                    return true;
                } else {
                    println!("Got wrong size. Got {} expected {}", size, STRUCT_SIZE);
                }
            }

            Err(e) => println!("Error in update gw2: {e}"),
        }
        false
    }

    pub fn get_gw2_data(&self) -> Box<LinkedMem> {
        let copy: LinkedMem = unsafe { (*self.gw2_data.mem).clone() };
        println!("{:?}", copy.get_identity());
        Box::new(copy)
    }
}

#[cfg(test)]
mod tests {
    use super::MumbleContext;

    #[test]
    fn test_ui_state() {
        let mut ctx = MumbleContext {
            ..Default::default()
        };

        ctx.ui_state = 1 << 0;
        assert!(ctx.get_ui_state(1 << 0));
        assert!(!ctx.get_ui_state(1 << 1));

        for i in 0..7 {
            ctx.ui_state = 1 << i;
            assert!(ctx.get_ui_state(1 << i));
        }
    }
}

//! serialport-rs is a cross-platform serial port library.
//!
//! The goal of this library is to expose a cross-platform and platform-specific API for enumerating
//! and using blocking I/O with serial ports. This library exposes a similar API to that provided
//! by [Qt's `QSerialPort` library](https://doc.qt.io/qt-5/qserialport.html).
//!
//! # Feature Overview
//!
//! The library has been organized such that there is a high-level `SerialPort` trait that provides
//! a cross-platform API for accessing serial ports. This is the preferred method of interacting
//! with ports and as such is part of the `prelude`. The `open*()` and `available_ports()` functions
//! in the root provide cross-platform functionality.
//!
//! For platform-specific functionaly, this crate is split into a `posix` and `windows` API with
//! corresponding `TTYPort` and `COMPort` structs (that both implement the `SerialPort` trait).
//! Using the platform-specific `open*()` functions will return the platform-specific port object
//! which allows access to platform-specific functionality.

#![deny(missing_docs,
        missing_debug_implementations,
        missing_copy_implementations,
        unused_import_braces,
        unused_allocation,
        unused_qualifications)]
// Don't worry about needing to `unwrap()` or otherwise handle some results in
// doc tests.
#![doc(test(attr(allow(unused_must_use))))]

extern crate libc;
#[cfg(target_os = "linux")]
extern crate libudev;
#[cfg(unix)]
extern crate nix;
#[cfg(unix)]
extern crate ioctl_rs as ioctl;
#[cfg(target_os = "macos")]
extern crate IOKit_sys;
#[cfg(target_os = "macos")]
extern crate CoreFoundation_sys as cf;
#[cfg(target_os = "macos")]
extern crate mach;
#[cfg(unix)]
extern crate termios;

#[cfg(windows)]
extern crate advapi32;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate regex;
#[cfg(windows)]
extern crate setupapi;
#[cfg(windows)]
extern crate winapi;

use std::error::Error as StdError;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;
use std::time::Duration;

/// A module that exports types that are useful to have in scope.
///
/// It is intended to be glob imported:
///
/// ```
/// # #[allow(unused_imports)]
/// use serialport::prelude::*;
/// ```
pub mod prelude {
    pub use {BaudRate, DataBits, FlowControl, Parity, StopBits};
    pub use {SerialPort, SerialPortInfo, SerialPortSettings};
}

#[cfg(unix)]
/// The implementation of serialport for POSIX-based systems (Linux, BSD, Mac)
pub mod posix;

#[cfg(windows)]
/// The implementation of serialport for Windows systems
pub mod windows;

/// A type for results generated by interacting with serial ports.
///
/// The `Err` type is hard-wired to [`serialport::Error`](struct.Error.html).
pub type Result<T> = std::result::Result<T, ::Error>;

/// Categories of errors that can occur when interacting with serial ports.
///
/// This list is intended to grow over time and it is not recommended to
/// exhaustively match against it.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ErrorKind {
    /// The device is not available.
    ///
    /// This could indicate that the device is in use by another process or was
    /// disconnected while performing I/O.
    NoDevice,

    /// A parameter was incorrect.
    InvalidInput,

    /// An unknown error occurred.
    Unknown,

    /// An I/O error occurred.
    ///
    /// The type of I/O error is determined by the inner `io::ErrorKind`.
    Io(io::ErrorKind),
}

/// An error type for serial port operations.
#[derive(Debug)]
pub struct Error {
    /// The kind of error this is
    pub kind: ErrorKind,
    /// A description of the error suitable for end-users
    pub description: String,
}

impl Error {
    /// Instantiates a new error
    pub fn new<T: Into<String>>(kind: ErrorKind, description: T) -> Self {
        Error {
            kind: kind,
            description: description.into(),
        }
    }

    /// Returns the corresponding `ErrorKind` for this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        fmt.write_str(&self.description)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        &self.description
    }
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Error {
        Error::new(ErrorKind::Io(io_error.kind()), format!("{}", io_error))
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        let kind = match error.kind {
            ErrorKind::NoDevice => io::ErrorKind::NotFound,
            ErrorKind::InvalidInput => io::ErrorKind::InvalidInput,
            ErrorKind::Unknown => io::ErrorKind::Other,
            ErrorKind::Io(kind) => kind,
        };

        io::Error::new(kind, error.description)
    }
}

/// Serial port baud rates.
///
/// ## Portability
///
/// The `BaudRate` variants with numeric suffixes, e.g., `Baud9600`, indicate standard baud rates
/// that are widely-supported on many systems. While non-standard baud rates can be set with
/// `BaudOther`, their behavior is system-dependent. Some systems may not support arbitrary baud
/// rates. Using the standard baud rates is more likely to result in portable applications.
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum BaudRate {
    /** 110 baud. */
    Baud110,
    /** 300 baud. */
    Baud300,
    /** 600 baud. */
    Baud600,
    /** 1200 baud. */
    Baud1200,
    /** 2400 baud. */
    Baud2400,
    /** 4800 baud. */
    Baud4800,
    /** 9600 baud. */
    Baud9600,
    /** 19,200 baud. */
    Baud19200,
    /** 38,400 baud. */
    Baud38400,
    /** 57,600 baud. */
    Baud57600,
    /** 115,200 baud. */
    Baud115200,

    /// Non-standard baud rates.
    ///
    /// `BaudOther` can be used to set non-standard baud rates by setting its member to be the
    /// desired baud rate.
    ///
    /// ```
    /// # use serialport::BaudRate::BaudOther;
    /// BaudOther(4_000_000); // 4,000,000 baud
    /// ```
    ///
    /// Non-standard baud rates may not be supported on all systems.
    BaudOther(usize),
}

impl BaudRate {
    /// Creates a `BaudRate` for a particular speed.
    ///
    /// This function can be used to select a `BaudRate` variant from an integer containing the
    /// desired baud rate.
    ///
    /// ## Example
    ///
    /// ```
    /// # use serialport::BaudRate;
    /// assert_eq!(BaudRate::Baud9600, BaudRate::from_speed(9600));
    /// assert_eq!(BaudRate::Baud115200, BaudRate::from_speed(115200));
    /// assert_eq!(BaudRate::BaudOther(4000000), BaudRate::from_speed(4000000));
    /// ```
    pub fn from_speed(speed: usize) -> BaudRate {
        match speed {
            110 => BaudRate::Baud110,
            300 => BaudRate::Baud300,
            600 => BaudRate::Baud600,
            1200 => BaudRate::Baud1200,
            2400 => BaudRate::Baud2400,
            4800 => BaudRate::Baud4800,
            9600 => BaudRate::Baud9600,
            19200 => BaudRate::Baud19200,
            38400 => BaudRate::Baud38400,
            57600 => BaudRate::Baud57600,
            115200 => BaudRate::Baud115200,
            n => BaudRate::BaudOther(n),
        }
    }

    /// Returns the baud rate as an integer.
    ///
    /// ## Example
    ///
    /// ```
    /// # use serialport::BaudRate;
    /// assert_eq!(9600, BaudRate::Baud9600.speed());
    /// assert_eq!(115200, BaudRate::Baud115200.speed());
    /// assert_eq!(4000000, BaudRate::BaudOther(4000000).speed());
    /// ```
    pub fn speed(&self) -> usize {
        match *self {
            BaudRate::Baud110 => 110,
            BaudRate::Baud300 => 300,
            BaudRate::Baud600 => 600,
            BaudRate::Baud1200 => 1200,
            BaudRate::Baud2400 => 2400,
            BaudRate::Baud4800 => 4800,
            BaudRate::Baud9600 => 9600,
            BaudRate::Baud19200 => 19200,
            BaudRate::Baud38400 => 38400,
            BaudRate::Baud57600 => 57600,
            BaudRate::Baud115200 => 115200,
            BaudRate::BaudOther(n) => n,
        }
    }
}

/// Number of bits per character.
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum DataBits {
    /// 5 bits per character
    Five,

    /// 6 bits per character
    Six,

    /// 7 bits per character
    Seven,

    /// 8 bits per character
    Eight,
}

/// Parity checking modes.
///
/// When parity checking is enabled (`Odd` or `Even`) an extra bit is transmitted with
/// each character. The value of the parity bit is arranged so that the number of 1 bits in the
/// character (including the parity bit) is an even number (`Even`) or an odd number
/// (`Odd`).
///
/// Parity checking is disabled by setting `None`, in which case parity bits are not
/// transmitted.
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum Parity {
    /// No parity bit.
    None,

    /// Parity bit sets odd number of 1 bits.
    Odd,

    /// Parity bit sets even number of 1 bits.
    Even,
}

/// Number of stop bits.
///
/// Stop bits are transmitted after every character.
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum StopBits {
    /// One stop bit.
    One,

    /// Two stop bits.
    Two,
}

/// Flow control modes.
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum FlowControl {
    /// No flow control.
    None,

    /// Flow control using XON/XOFF bytes.
    Software,

    /// Flow control using RTS/CTS signals.
    Hardware,
}

/// A struct containing all serial port settings
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub struct SerialPortSettings {
    /// The baud rate in symbols-per-second
    pub baud_rate: BaudRate,
    /// Number of bits used to represent a character sent on the line
    pub data_bits: DataBits,
    /// The type of signalling to use for controlling data transfer
    pub flow_control: FlowControl,
    /// The type of parity to use for error checking
    pub parity: Parity,
    /// Number of bits to use to signal the end of a character
    pub stop_bits: StopBits,
    /// Amount of time to wait to receive data before timing out
    pub timeout: Duration,
}

impl Default for SerialPortSettings {
    fn default() -> SerialPortSettings {
        SerialPortSettings {
            baud_rate: BaudRate::Baud9600,
            data_bits: DataBits::Eight,
            flow_control: FlowControl::None,
            parity: Parity::None,
            stop_bits: StopBits::One,
            timeout: Duration::from_millis(1),
        }
    }
}

/// A trait for serial port devices
///
/// This trait is all that's necessary to implement a new serial port driver
/// for a new platform.
pub trait SerialPort: io::Read + io::Write {
    // Port settings getters

    /// Returns the name of this port if it exists.
    ///
    /// This name may not be the canonical device name and instead be shorthand.
    /// Additionally it may not exist for virtual ports.
    fn port_name(&self) -> Option<String>;

    /// Returns a struct with the current port settings
    fn settings(&self) -> SerialPortSettings;

    /// Returns the current baud rate.
    ///
    /// This function returns `None` if the baud rate could not be determined. This may occur if
    /// the hardware is in an uninitialized state. Setting a baud rate with `set_baud_rate()`
    /// should initialize the baud rate to a supported value.
    fn baud_rate(&self) -> Option<BaudRate>;

    /// Returns the character size.
    ///
    /// This function returns `None` if the character size could not be determined. This may occur
    /// if the hardware is in an uninitialized state or is using a non-standard character size.
    /// Setting a baud rate with `set_char_size()` should initialize the character size to a
    /// supported value.
    fn data_bits(&self) -> Option<DataBits>;

    /// Returns the flow control mode.
    ///
    /// This function returns `None` if the flow control mode could not be determined. This may
    /// occur if the hardware is in an uninitialized state or is using an unsupported flow control
    /// mode. Setting a flow control mode with `set_flow_control()` should initialize the flow
    /// control mode to a supported value.
    fn flow_control(&self) -> Option<FlowControl>;

    /// Returns the parity-checking mode.
    ///
    /// This function returns `None` if the parity mode could not be determined. This may occur if
    /// the hardware is in an uninitialized state or is using a non-standard parity mode. Setting
    /// a parity mode with `set_parity()` should initialize the parity mode to a supported value.
    fn parity(&self) -> Option<Parity>;

    /// Returns the number of stop bits.
    ///
    /// This function returns `None` if the number of stop bits could not be determined. This may
    /// occur if the hardware is in an uninitialized state or is using an unsupported stop bit
    /// configuration. Setting the number of stop bits with `set_stop-bits()` should initialize the
    /// stop bits to a supported value.
    fn stop_bits(&self) -> Option<StopBits>;

    /// Returns the current timeout.
    fn timeout(&self) -> Duration;

    // Port settings setters

    /// Applies all settings for a struct. This isn't guaranteed to involve only
    /// a single call into the driver, though that may be done on some
    /// platforms.
    fn set_all(&mut self, &SerialPortSettings) -> ::Result<()>;

    /// Sets the baud rate.
    ///
    /// ## Errors
    ///
    /// If the implementation does not support the requested baud rate, this function may return an
    /// `InvalidInput` error. Even if the baud rate is accepted by `set_baud_rate()`, it may not be
    /// supported by the underlying hardware.
    fn set_baud_rate(&mut self, baud_rate: BaudRate) -> ::Result<()>;

    /// Sets the character size.
    fn set_data_bits(&mut self, data_bits: DataBits) -> ::Result<()>;

    /// Sets the flow control mode.
    fn set_flow_control(&mut self, flow_control: FlowControl) -> ::Result<()>;

    /// Sets the parity-checking mode.
    fn set_parity(&mut self, parity: Parity) -> ::Result<()>;

    /// Sets the number of stop bits.
    fn set_stop_bits(&mut self, stop_bits: StopBits) -> ::Result<()>;

    /// Sets the timeout for future I/O operations.
    fn set_timeout(&mut self, timeout: Duration) -> ::Result<()>;

    // Functions for setting non-data control signal pins

    /// Sets the state of the RTS (Request To Send) control signal.
    ///
    /// Setting a value of `true` asserts the RTS control signal. `false` clears the signal.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the RTS control signal could not be set to the desired
    /// state on the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn write_request_to_send(&mut self, level: bool) -> ::Result<()>;

    /// Writes to the Data Terminal Ready pin
    ///
    /// Setting a value of `true` asserts the DTR control signal. `false` clears the signal.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the DTR control signal could not be set to the desired
    /// state on the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn write_data_terminal_ready(&mut self, level: bool) -> ::Result<()>;

    // Functions for reading additional pins

    /// Reads the state of the CTS (Clear To Send) control signal.
    ///
    /// This function returns a boolean that indicates whether the CTS control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the CTS control signal could not be read
    /// from the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_clear_to_send(&mut self) -> ::Result<bool>;

    /// Reads the state of the Data Set Ready control signal.
    ///
    /// This function returns a boolean that indicates whether the DSR control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the DSR control signal could not be read
    /// from the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_data_set_ready(&mut self) -> ::Result<bool>;

    /// Reads the state of the Ring Indicator control signal.
    ///
    /// This function returns a boolean that indicates whether the RI control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the RI control signal could not be read from
    /// the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_ring_indicator(&mut self) -> ::Result<bool>;

    /// Reads the state of the Carrier Detect control signal.
    ///
    /// This function returns a boolean that indicates whether the CD control signal is asserted.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the state of the CD control signal could not be read from
    /// the underlying hardware:
    ///
    /// * `NoDevice` if the device was disconnected.
    /// * `Io` for any other type of I/O error.
    fn read_carrier_detect(&mut self) -> ::Result<bool>;
}

#[derive(Debug,Clone,PartialEq,Eq)]
/// Contains all possible USB information about a `SerialPort`
pub struct UsbPortInfo {
    /// Vender ID
    pub vid: u16,
    /// Product ID
    pub pid: u16,
    /// Serial number (arbitrary string)
    pub serial_number: Option<String>,
    /// Manufacturer (arbitrary string)
    pub manufacturer: Option<String>,
    /// Product name (arbitrary string)
    pub product: Option<String>,
}

#[derive(Debug,Clone,PartialEq,Eq)]
/// The physical type of a `SerialPort`
pub enum SerialPortType {
    /// The serial port is connected via USB
    UsbPort(UsbPortInfo),
    /// The serial port is connected via PCI (permanent port)
    PciPort,
    /// It can't be determined how the serial port is connected
    Unknown,
}

/// A device-independent implementation of serial port information.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SerialPortInfo {
    /// The short name of the serial port
    pub port_name: String,
    /// The hardware device type that exposes this port
    pub port_type: SerialPortType,
}

/// Opens the serial port specified by the device path using default settings.
///
/// The default settings are:
///
/// * Baud: 9600
/// * Data bits: 8
/// * Flow control: None
/// * Parity: None
/// * Stop bits: 1
/// * Timeout: 1ms
///
/// This is the canonical way to open a new serial port.
///
/// ```
/// serialport::open("/dev/ttyUSB0");
/// ```
pub fn open<T: AsRef<OsStr> + ?Sized>(port: &T) -> ::Result<Box<SerialPort>> {
    // This is written with explicit returns because of:
    // https://github.com/rust-lang/rust/issues/38337

    #[cfg(unix)]
    return match posix::TTYPort::open(Path::new(port), &Default::default()) {
               Ok(p) => Ok(Box::new(p)),
               Err(e) => Err(e),
           };

    #[cfg(windows)]
    return match windows::COMPort::open(Path::new(port), &Default::default()) {
               Ok(p) => Ok(Box::new(p)),
               Err(e) => Err(e),
           };

    #[cfg(not(any(unix, windows)))]
    Err(Error::new(ErrorKind::Unknown, "open() not implemented for platform"))
}

/// Opens the serial port specified by the device path with the given settings.
///
/// ```
/// use serialport::prelude::*;
/// use std::time::Duration;
///
/// let s = SerialPortSettings {
///     baud_rate: BaudRate::Baud9600,
///     data_bits: DataBits::Eight,
///     flow_control: FlowControl::None,
///     parity: Parity::None,
///     stop_bits: StopBits::One,
///     timeout: Duration::from_millis(1),
/// };
/// serialport::open_with_settings("/dev/ttyUSB0", &s);
/// ```
pub fn open_with_settings<T: AsRef<OsStr> + ?Sized>(port: &T,
                                                    settings: &SerialPortSettings)
                                                    -> ::Result<Box<SerialPort>> {
    // This is written with explicit returns because of:
    // https://github.com/rust-lang/rust/issues/38337

    #[cfg(unix)]
    return match posix::TTYPort::open(Path::new(port), settings) {
               Ok(p) => Ok(Box::new(p)),
               Err(e) => Err(e),
           };

    #[cfg(windows)]
    return match windows::COMPort::open(port, settings) {
               Ok(p) => Ok(Box::new(p)),
               Err(e) => Err(e),
           };

    #[cfg(not(any(unix, windows)))]
    Err(Error::new(ErrorKind::Unknown, "open() not implemented for platform"))
}

/// Returns a list of all serial ports on system
///
/// It is not guaranteed that these ports exist or are available even if they're
/// returned by this function.
pub fn available_ports() -> ::Result<Vec<SerialPortInfo>> {
    #[cfg(unix)]
    return posix::available_ports();

    #[cfg(windows)]
    return windows::available_ports();

    #[cfg(not(any(unix, windows)))]
    Err(Error::new(ErrorKind::Unknown,
                   "available_ports() not implemented for platform"))
}

/// Returns a list of available baud rates for this system.
///
/// Officially supported baud rates vary by system so this function may not
/// return the same results across all platforms. Also note that this list may
/// be incomplete and it's likely that the underlying platform and hardware can
/// support more baud rates than returned by this function.
pub fn available_baud_rates() -> Vec<u32> {
    #[cfg(unix)]
    return posix::available_baud_rates();

    #[cfg(windows)]
    return windows::available_baud_rates();

    #[cfg(not(any(unix, windows)))]
    return Vec::new();
}

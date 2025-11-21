use super::*;
use icy_parser_core::{AnsiParser, CommandParser, TerminalRequest};

#[test]
fn test_macro_checksum_report() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Load two macros: macro 0 with "Hello", macro 1 with "World"
    parser.parse(b"\x1BP0;0;0!zHello\x1B\\", &mut sink);
    parser.parse(b"\x1BP1;0;0!zWorld\x1B\\", &mut sink);

    // Request memory checksum report with pid=1
    parser.parse(b"\x1B[?63;1n", &mut sink);

    // Should have one request
    assert_eq!(sink.requests.len(), 1);

    // Check that we got a MemoryChecksumReport with the correct checksum
    if let TerminalRequest::MemoryChecksumReport(pid, checksum) = sink.requests[0] {
        assert_eq!(pid, 1);
        // Checksum calculation: "Hello" = 72+101+108+108+111 = 500 (0x01F4)
        //                       "World" = 87+111+114+108+100 = 520 (0x0208)
        //                       Total = 1020 (0x03FC)
        assert_eq!(checksum, 0x03FC);
    } else {
        panic!("Expected MemoryChecksumReport");
    }
}

#[test]
fn test_csi_dollar_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps$w - Request Tab Stop Report
    parser.parse(b"\x1B[2$w", &mut sink);
    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::RequestTabStopReport = sink.requests[0] {
        // Success
    } else {
        panic!("Expected RequestTabStopReport");
    }
}

#[test]
fn test_rect_checksum_decrqcra() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn ; Pid ; Pp ; Pr ; Pc ; Pp *y - Request Checksum of Rectangular Area (DECRQCRA)
    parser.parse(b"\x1B[88;42;1;2;10;20*y", &mut sink);

    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::RequestChecksumRectangularArea {
        id,
        page,
        top,
        left,
        bottom,
        right,
    } = sink.requests[0]
    {
        assert_eq!(id, 88);
        assert_eq!(page, 42);
        assert_eq!(top, 1);
        assert_eq!(left, 2);
        assert_eq!(bottom, 10);
        assert_eq!(right, 20);
    } else {
        panic!("Expected RequestChecksumRectangularArea");
    }
}

#[test]
fn test_macro_space_report() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI ?62n - Request Macro Space Report
    parser.parse(b"\x1B[?62n", &mut sink);

    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::MacroSpaceReport = sink.requests[0] {
        // Success - correct request type
    } else {
        panic!("Expected MacroSpaceReport");
    }
}

#[test]
fn test_request_tab_stop_report() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 2$w - Request Tab Stop Report (DECRQTSR)
    parser.parse(b"\x1B[2$w", &mut sink);

    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::RequestTabStopReport = sink.requests[0] {
        // Success
    } else {
        panic!("Expected RequestTabStopReport");
    }
}

#[test]
fn test_font_state_report() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI =1n - Request Font State Report
    parser.parse(b"\x1B[=1n", &mut sink);
    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::FontStateReport = sink.requests[0] {
        // Success
    } else {
        panic!("Expected FontStateReport");
    }

    sink.requests.clear();

    // CSI =2n - Request Font Mode Report
    parser.parse(b"\x1B[=2n", &mut sink);
    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::FontModeReport = sink.requests[0] {
        // Success
    } else {
        panic!("Expected FontModeReport");
    }

    sink.requests.clear();

    // CSI =3n - Request Font Dimension Report
    parser.parse(b"\x1B[=3n", &mut sink);
    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::FontDimensionReport = sink.requests[0] {
        // Success
    } else {
        panic!("Expected FontDimensionReport");
    }
}

#[test]
fn test_extended_device_attributes() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI <0c - Request Extended Device Attributes
    parser.parse(b"\x1B[<0c", &mut sink);

    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::ExtendedDeviceAttributes = sink.requests[0] {
        // Success
    } else {
        panic!("Expected ExtendedDeviceAttributes");
    }
}

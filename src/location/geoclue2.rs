/// Geoclue2 support
use super::Location;
use dbus::{Connection, BusType, ConnectionItem};

const GEOCLUE: &'static str = "org.freedesktop.GeoClue2";

pub fn location() -> Location {

    let c = Connection::get_private(BusType::System).unwrap();
    c.add_match(GEOCLUE);

    for (i, c) in c.iter(1000).enumerate() {
        match c {
            ConnectionItem::MethodCall(ref msg) => {
                println!("method: {:?}", msg);
            }
            ConnectionItem::Signal(ref msg) => {
                println!("signal: {:?}", msg);
            }
            ConnectionItem::MethodReturn(ref msg) => {
                println!("return: {:?}", msg);
            }
            _ => {}
        }
    }

    Location {
        lat: 55.7,
        lon: 12.6,
    }
}

#[cfg(test)]
mod test {
    use dbus::{Connection, BusType, Message};
    use dbus::arg::Array;
    use super::GEOCLUE;

    #[test]
    fn use_location() {
        //let _ = super::location();

        let c = Connection::get_private(BusType::System).unwrap();
        //let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
        let m = Message::new_method_call("org.freedesktop.GeoClue2", "/org/freedesktop/GeoClue2/Manager", "org.freedesktop.GeoClue2.Manager", "GetClient").unwrap();
        let r = c.send_with_reply_and_block(m, 2000).unwrap();

        println!("reply: {:?} from sender {:?}", r, r.sender());

        // ListNames returns one argument, which is an array of strings.

        // let arr: Array<&str, _>  = r.get1().unwrap();
        // println!("List of names:");
        // for name in arr {
        //     println!("  {}", name);
        // }
    }
}

use std::any::Any;
pub trait Interconnect: InterconnectClone{
    fn load8(&self, addr:u16)->Option<u8>;
    fn store8(&mut self, addr:u16,val:u8);
    fn as_any(&self) -> &dyn Any;
}
pub trait InterconnectClone {
    fn clone_box(&self) -> Box<dyn Interconnect>;
}
impl<T> InterconnectClone for T 
where 
    T: 'static + Interconnect + Clone,
{
    fn clone_box(&self) -> Box<dyn Interconnect>{
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn Interconnect>{
    fn clone(&self) -> Box<dyn Interconnect>{
        self.clone_box()
    }
}

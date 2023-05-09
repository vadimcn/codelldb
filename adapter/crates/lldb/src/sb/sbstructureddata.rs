use super::*;

cpp_class!(pub unsafe struct SBStructuredData as "SBStructuredData");

unsafe impl Send for SBStructuredData {}

impl SBStructuredData {}

impl IsValid for SBStructuredData {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBStructuredData*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBStructuredData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBStructuredData*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr).Success();
            })
        })
    }
}

use syn::fold::Fold;

struct SelfReplacer {
    self_type: syn::Type,
}

impl Fold for SelfReplacer {
    fn fold_type(
        &mut self,
        ty: syn::Type,
    ) -> syn::Type {
        match ty {
            syn::Type::Path(ref path)
                if path.qself.is_none()
                    && path.path.segments.len() == 1
                    && path.path.segments[0].ident == "Self"
                    && path.path.segments[0].arguments.is_empty() =>
            {
                self.self_type.clone()
            },
            ty => syn::fold::fold_type(self, ty),
        }
    }
}

pub fn replace_self_in_return(
    output: &syn::ReturnType,
    self_type: &syn::Type,
) -> syn::ReturnType {
    let mut folder = SelfReplacer {
        self_type: self_type.clone(),
    };
    folder.fold_return_type(output.clone())
}

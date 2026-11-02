EDITS = [
("""                Some(t) => {
                    let ty = self.resolve(t);
                    if let Some(wanted) = wanted_params.as_ref().map(|ps| &ps[i])
                        && !compatible(wanted, &ty)
                    {
                        self.mismatch(param.span, wanted, &ty);
                    }
                    Some(ty)
                }
                None => wanted_params.as_ref().map(|ps| ps[i].clone()),""",
"""                Some(t) => Some(self.resolve(t)),
                None => wanted_params.as_ref().map(|ps| ps[i].clone()),"""),
]

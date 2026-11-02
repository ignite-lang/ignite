EDITS = [
("""        let mut params: Vec<(String, Ty)> = Vec::new();
        let mut known = true;""",
"""        let mut params: Vec<(String, Ty)> = Vec::new();
        let mut bound: Vec<Option<Ty>> = Vec::new();
        let mut known = true;"""),
("""            match ty.filter(|t| !t.has_param(&is_var)) {
                Some(ty) => params.push((param.name.name.clone(), ty)),
                None => {""",
"""            let ty = ty.filter(|t| !t.has_param(&is_var));
            bound.push(ty.clone());
            match ty {
                Some(ty) => params.push((param.name.name.clone(), ty)),
                None => {"""),
("""        for (param, (_, ty)) in closure.params.iter().zip(&params) {
            self.bind(&param.name, Some(ty.clone()));
        }""",
"""        for (param, ty) in closure.params.iter().zip(bound) {
            self.bind(&param.name, ty);
        }"""),
]

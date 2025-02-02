use numpy::{PyArray2, ToPyArray};
use quil_rs::{
    expression::Expression,
    instruction::{
        Gate, GateDefinition, GateModifier, GateSpecification, PauliGate, PauliSum, PauliTerm,
        Qubit,
    },
};
use rigetti_pyo3::{
    impl_from_str, impl_hash, impl_parse, impl_repr, impl_str,
    num_complex::Complex64,
    py_wrap_data_struct, py_wrap_error, py_wrap_simple_enum, py_wrap_type, py_wrap_union_enum,
    pyo3::{
        exceptions::PyValueError,
        pyclass::CompareOp,
        pymethods,
        types::{PyInt, PyString},
        IntoPy, Py, PyErr, PyObject, PyResult, Python,
    },
    wrap_error, PyTryFrom, PyWrapper, PyWrapperMut, ToPython, ToPythonError,
};
use strum;

use crate::{expression::PyExpression, impl_to_quil, instruction::PyQubit};

wrap_error!(RustGateError(quil_rs::instruction::GateError));
py_wrap_error!(quil, RustGateError, GateError, PyValueError);
wrap_error!(RustParseEnumError(strum::ParseError));
py_wrap_error!(quil, RustParseEnumError, EnumParseError, PyValueError);

py_wrap_data_struct! {
    #[derive(Debug, PartialEq, Eq)]
    #[pyo3(subclass)]
    PyGate(Gate) as "Gate" {
        name: String => Py<PyString>,
        parameters: Vec<Expression> => Vec<PyExpression>,
        qubits: Vec<Qubit> => Vec<PyQubit>,
        modifiers: Vec<GateModifier> => Vec<PyGateModifier>
    }
}
impl_repr!(PyGate);
impl_to_quil!(PyGate);
impl_hash!(PyGate);

#[pymethods]
impl PyGate {
    #[new]
    fn new(
        py: Python<'_>,
        name: String,
        parameters: Vec<PyExpression>,
        qubits: Vec<PyQubit>,
        modifiers: Vec<PyGateModifier>,
    ) -> PyResult<Self> {
        Ok(Self(
            Gate::new(
                &name,
                Vec::<Expression>::py_try_from(py, &parameters)?,
                Vec::<Qubit>::py_try_from(py, &qubits)?,
                Vec::<GateModifier>::py_try_from(py, &modifiers)?,
            )
            .map_err(RustGateError::from)
            .map_err(RustGateError::to_py_err)?,
        ))
    }

    fn dagger(&self, py: Python<'_>) -> PyResult<Self> {
        self.as_inner().clone().dagger().to_python(py)
    }

    fn controlled(&self, py: Python<'_>, control_qubit: PyQubit) -> PyResult<Self> {
        self.as_inner()
            .clone()
            .controlled(Qubit::py_try_from(py, &control_qubit)?)
            .to_python(py)
    }

    fn forked(
        &self,
        py: Python<'_>,
        fork_qubit: PyQubit,
        params: Vec<PyExpression>,
    ) -> PyResult<Self> {
        self.as_inner()
            .clone()
            .forked(
                Qubit::py_try_from(py, &fork_qubit)?,
                Vec::<Expression>::py_try_from(py, &params)?,
            )
            .map_err(RustGateError::from)
            .map_err(RustGateError::to_py_err)?
            .to_python(py)
    }

    fn to_unitary_mut(
        &mut self,
        py: Python<'_>,
        n_qubits: u64,
    ) -> PyResult<Py<PyArray2<Complex64>>> {
        Ok(self
            .as_inner_mut()
            .to_unitary(n_qubits)
            .map_err(RustGateError::from)
            .map_err(RustGateError::to_py_err)?
            .to_pyarray(py)
            .to_owned())
    }

    pub fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> PyObject {
        match op {
            CompareOp::Eq => (self.as_inner() == other.as_inner()).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

py_wrap_simple_enum! {
    #[derive(Debug, PartialEq, Eq)]
    PyGateModifier(GateModifier) as "GateModifier" {
        Controlled,
        Dagger,
        Forked
    }
}
impl_repr!(PyGateModifier);
impl_to_quil!(PyGateModifier);
impl_hash!(PyGateModifier);

#[pymethods]
impl PyGateModifier {
    pub fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> PyObject {
        match op {
            CompareOp::Eq => (self.as_inner() == other.as_inner()).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

py_wrap_simple_enum! {
    #[derive(Debug, PartialEq, Eq)]
    PyPauliGate(PauliGate) as "PauliGate" {
        I,
        X,
        Y,
        Z
    }
}
impl_repr!(PyPauliGate);
impl_str!(PyPauliGate);
impl_hash!(PyPauliGate);
impl_from_str!(PyPauliGate, RustParseEnumError);
impl_parse!(PyPauliGate);

// This is a helper type to help manage easy conversion of the inner tuple
// with the macros. It should not be exposed directly.
py_wrap_type! {
    PyPauliPair((PauliGate, String))
}

impl PyPauliPair {
    pub(crate) fn from_py_tuple(py: Python<'_>, tuple: (PyPauliGate, String)) -> PyResult<Self> {
        Ok(Self((PauliGate::py_try_from(py, &tuple.0)?, tuple.1)))
    }
}

py_wrap_data_struct! {
    #[derive(Debug, PartialEq, Eq)]
    #[pyo3(subclass)]
    PyPauliTerm(PauliTerm) as "PauliTerm" {
        arguments: Vec<(PauliGate, String)> => Vec<PyPauliPair>,
        expression: Expression => PyExpression
    }
}

#[pymethods]
impl PyPauliTerm {
    #[new]
    pub fn new(
        py: Python<'_>,
        arguments: Vec<(PyPauliGate, String)>,
        expression: PyExpression,
    ) -> PyResult<Self> {
        Ok(Self(PauliTerm::new(
            Vec::<(PauliGate, String)>::py_try_from(
                py,
                &PyPauliTerm::py_pairs_from_tuples(py, arguments)?,
            )?,
            Expression::py_try_from(py, &expression)?,
        )))
    }

    // Override the getters/setters generated by [`py_wrap_data_struct!`] so that they
    // return/take tuples instead of the wrapping [`PyPauliPair`] type.
    #[getter(arguments)]
    fn get_arguments_as_tuple(&self, py: Python<'_>) -> PyResult<Vec<(PyPauliGate, String)>> {
        let mut pairs: Vec<(PyPauliGate, String)> =
            Vec::with_capacity(self.as_inner().arguments.len());
        self.as_inner()
            .arguments
            .iter()
            .try_for_each(|(gate, arg)| {
                pairs.push((gate.to_python(py)?, arg.clone()));
                Ok::<(), PyErr>(())
            })?;
        Ok(pairs)
    }

    #[setter(arguments)]
    fn set_arguments_from_tuple(
        &mut self,
        py: Python<'_>,
        arguments: Vec<(PyPauliGate, String)>,
    ) -> PyResult<()> {
        self.as_inner_mut().arguments = Vec::<(PauliGate, String)>::py_try_from(
            py,
            &PyPauliTerm::py_pairs_from_tuples(py, arguments)?,
        )?;
        Ok(())
    }
}

impl PyPauliTerm {
    pub(crate) fn py_pairs_from_tuples(
        py: Python<'_>,
        tuples: Vec<(PyPauliGate, String)>,
    ) -> PyResult<Vec<PyPauliPair>> {
        let mut pairs: Vec<PyPauliPair> = Vec::with_capacity(tuples.len());
        tuples.into_iter().try_for_each(|tuple| {
            pairs.push(PyPauliPair::from_py_tuple(py, tuple)?);
            Ok::<(), PyErr>(())
        })?;
        Ok(pairs)
    }
}

py_wrap_data_struct! {
    #[derive(Debug, PartialEq, Eq)]
    #[pyo3(subclass)]
    PyPauliSum(PauliSum) as "PauliSum" {
        arguments: Vec<String> => Vec<Py<PyString>>,
        terms: Vec<PauliTerm> => Vec<PyPauliTerm>
    }
}
impl_repr!(PyPauliSum);

#[pymethods]
impl PyPauliSum {
    #[new]
    pub fn new(py: Python<'_>, arguments: Vec<String>, terms: Vec<PyPauliTerm>) -> PyResult<Self> {
        Ok(Self(
            PauliSum::new(arguments, Vec::<PauliTerm>::py_try_from(py, &terms)?)
                .map_err(RustGateError::from)
                .map_err(RustGateError::to_py_err)?,
        ))
    }

    pub fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> PyObject {
        match op {
            CompareOp::Eq => (self.as_inner() == other.as_inner()).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

py_wrap_union_enum! {
    #[derive(Debug, PartialEq, Eq)]
    PyGateSpecification(GateSpecification) as "GateSpecification" {
        matrix: Matrix => Vec<Vec<PyExpression>>,
        permutation: Permutation => Vec<Py<PyInt>>,
        pauli_sum: PauliSum => PyPauliSum
    }
}
impl_repr!(PyGateSpecification);
impl_to_quil!(PyGateSpecification);
impl_hash!(PyGateSpecification);

#[pymethods]
impl PyGateSpecification {
    pub fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> PyObject {
        match op {
            CompareOp::Eq => (self.as_inner() == other.as_inner()).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

py_wrap_data_struct! {
    #[derive(Debug, PartialEq, Eq)]
    #[pyo3(subclass)]
    PyGateDefinition(GateDefinition) as "GateDefinition" {
        name: String => Py<PyString>,
        parameters: Vec<String> => Vec<Py<PyString>>,
        specification: GateSpecification => PyGateSpecification
    }
}
impl_repr!(PyGateDefinition);
impl_to_quil!(PyGateDefinition);
impl_hash!(PyGateDefinition);

#[pymethods]
impl PyGateDefinition {
    #[new]
    pub fn new(
        py: Python<'_>,
        name: String,
        parameters: Vec<String>,
        specification: PyGateSpecification,
    ) -> PyResult<Self> {
        Ok(Self(
            GateDefinition::new(
                name,
                parameters,
                GateSpecification::py_try_from(py, &specification)?,
            )
            .map_err(RustGateError::from)
            .map_err(RustGateError::to_py_err)?,
        ))
    }

    pub fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> PyObject {
        match op {
            CompareOp::Eq => (self.as_inner() == other.as_inner()).into_py(py),
            _ => py.NotImplemented(),
        }
    }
}

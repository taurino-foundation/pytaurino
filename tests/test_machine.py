from examples.machine import S_DATA, SM, imp_data


def test_machine_initial_state() -> None:
    data = imp_data().server_data()

    assert int(data[S_DATA.STATE_MACHINE.value]) == SM.INIT.value


def test_measurement_initial_values() -> None:
    data = imp_data().server_data()

    assert float(data[S_DATA.I.value]) == 0.0
    assert float(data[S_DATA.U.value]) == 0.0
    assert float(data[S_DATA.T_MEAS.value]) == 20000.0


def test_measurement_data_size() -> None:
    data = imp_data().server_data()

    assert len(data) == len(S_DATA)

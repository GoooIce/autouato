use tch::{CModule, IValue, Tensor};

fn main() {
    let mut model = CModule::load("./silero_vad.jit").unwrap();

    model.set_eval();

    let chunk = Tensor::zeros(&[0, 512], (tch::Kind::Double, tch::Device::Cpu));

    let speech_prob = match model.method_is("forward", &[IValue::Tensor(chunk), IValue::Int(16000)])
    {
        Ok(speech_prob) => speech_prob,
        Err(e) => {
            eprintln!("error: {:?}", e);
            return;
        }
    };

    let v1 = <Tensor>::try_from(speech_prob).unwrap();
    assert_eq!(v1.size(), &[0, 1]);
}
